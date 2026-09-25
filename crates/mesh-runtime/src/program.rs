//! A program, validated (D3's program validation, and the assembly rules
//! of docs/manual/templates.md), and the identities derived from it:
//! program identity, keys and handler identifiers (docs/manual/runtime.md).

use crate::diagnostic::{Location, RuntimeCode, RuntimeDiagnostic};
use crate::number::number_to_text;
use mesh_manifest::{Component, Manifest, Type};
use mesh_template::{Child, Element, Expression, Refusal, Template};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// The parts of a program, as a host holds them: the root component's
/// name, and each template's text (`template-v1`).
#[derive(Debug, Clone, Copy)]
pub struct Program<'a> {
    pub root: &'a str,
    pub templates: &'a [&'a str],
}

/// A program that passed validation.
pub(crate) struct Valid {
    pub manifest: Manifest,
    pub root: String,
    /// The templates, by component.
    pub templates: BTreeMap<String, Template>,
    pub identity: [u8; 32],
}

impl Valid {
    pub(crate) fn component(&self, name: &str) -> &Component {
        &self.manifest.components()[name]
    }

    pub(crate) fn is_composite(&self, component: &str) -> bool {
        self.templates.contains_key(component)
    }
}

/// D3's program validation: its steps in order, each reporting every
/// problem it finds, and the first step that reports anything ending it.
pub(crate) fn validate(
    program: &Program<'_>,
    model: &str,
) -> Result<Valid, Vec<RuntimeDiagnostic>> {
    // Step 1: the model.
    let manifest = mesh_manifest::load(model).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(RuntimeDiagnostic::from_manifest)
            .collect::<Vec<_>>()
    })?;

    // Step 2: every template well-formed, of a supported version, and
    // naming only what the model declares.
    let mut diagnostics = Vec::new();
    let mut read = Vec::new();
    for (index, text) in program.templates.iter().enumerate() {
        let component = readable_component(text);
        match mesh_template::from_json(text) {
            Err(Refusal::UnsupportedVersion(version)) => diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::UNSUPPORTED_FORMAT_VERSION,
                format!(
                    "the template is format version {version}; this runtime reads version {}",
                    mesh_template::VERSION
                ),
                Location::Template { index, component },
            )),
            Err(Refusal::Malformed(problems)) => diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::MALFORMED_TEMPLATE,
                format!(
                    "the template is malformed: {}",
                    problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                Location::Template { index, component },
            )),
            Ok(template) => {
                let problems = undeclared(&manifest, &template);
                if problems.is_empty() {
                    read.push((index, template));
                } else {
                    diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::MALFORMED_TEMPLATE,
                        format!(
                            "the template names what the model doesn't declare: {}",
                            problems.join("; ")
                        ),
                        Location::Template {
                            index,
                            component: Some(template.component.clone()),
                        },
                    ));
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // Step 3: every template checked against this model.
    let fingerprint = mesh_template::fingerprint(&manifest);
    for (index, template) in &read {
        if template.fingerprint != fingerprint {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::FINGERPRINT_MISMATCH,
                format!(
                    "the template of `{}` was checked against another model ({}, not {fingerprint}): recompile it",
                    template.component, template.fingerprint
                ),
                Location::Template { index: *index, component: Some(template.component.clone()) },
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // Step 4: the assembly rules.
    let diagnostics = assembly(&manifest, program.root, &read);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let identity = identity(program);
    let templates = read
        .into_iter()
        .map(|(_, template)| (template.component.clone(), template))
        .collect();
    Ok(Valid {
        manifest,
        root: program.root.to_string(),
        templates,
        identity,
    })
}

/// A malformed template's `component`, if it has a string one.
fn readable_component(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("component")?.as_str().map(str::to_string)
}

/// Every name in `template` the model doesn't declare with the kind the
/// template gives it (docs/manual/templates.md, "Names").
fn undeclared(manifest: &Manifest, template: &Template) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(own) = manifest.components().get(&template.component) else {
        return vec![format!("no component `{}`", template.component)];
    };
    walk_names(manifest, own, &template.root, &mut problems);
    problems
}

fn walk_names(manifest: &Manifest, own: &Component, element: &Element, problems: &mut Vec<String>) {
    let components = manifest.components();
    let Some(component) = components.get(&element.component) else {
        problems.push(format!("no component `{}`", element.component));
        return;
    };
    for prop in &element.props {
        if !component.props.contains_key(&prop.prop) {
            problems.push(format!(
                "`{}` has no prop `{}`",
                element.component, prop.prop
            ));
        }
        scope_names(own, &prop.value, problems);
    }
    for binding in &element.events {
        match component.events.get(&binding.event) {
            None => problems.push(format!(
                "`{}` has no event `{}`",
                element.component, binding.event
            )),
            Some(event) => {
                if event.payload.is_none() && binding.arguments.iter().any(uses_event) {
                    problems.push(format!(
                        "`{}`'s `{}` has no payload for `$event`",
                        element.component, binding.event
                    ));
                }
            }
        }
        match own.commands.get(&binding.command) {
            None => problems.push(format!("no command `{}`", binding.command)),
            Some(command) if command.parameters.len() != binding.arguments.len() => {
                problems.push(format!(
                    "`{}` takes {} arguments, not {}",
                    binding.command,
                    command.parameters.len(),
                    binding.arguments.len()
                ))
            }
            Some(_) => {}
        }
        for argument in &binding.arguments {
            scope_names(own, argument, problems);
        }
    }
    for child in &element.children {
        match child {
            Child::Text { .. } => {}
            Child::Expression { expression } => scope_names(own, expression, problems),
            Child::Element { element } => walk_names(manifest, own, element, problems),
        }
    }
}

fn uses_event(expression: &Expression) -> bool {
    let mut found = false;
    visit(expression, &mut |e| {
        found |= matches!(e, Expression::Event { .. })
    });
    found
}

fn scope_names(own: &Component, expression: &Expression, problems: &mut Vec<String>) {
    visit(expression, &mut |e| {
        if let Expression::Scope { name, .. } = e {
            if !own.scope.contains_key(name) {
                problems.push(format!("no scope name `{name}`"));
            }
        }
    });
}

/// Calls `f` on `expression` and every expression inside it.
pub(crate) fn visit(expression: &Expression, f: &mut impl FnMut(&Expression)) {
    f(expression);
    match expression {
        Expression::Literal { .. } | Expression::Scope { .. } | Expression::Event { .. } => {}
        Expression::Member { object, .. } => visit(object, f),
        Expression::Unary { operand, .. } => visit(operand, f),
        Expression::Binary { left, right, .. } => {
            visit(left, f);
            visit(right, f);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            visit(condition, f);
            visit(consequent, f);
            visit(alternate, f);
        }
        Expression::List { elements, .. } => elements.iter().for_each(|e| visit(e, f)),
        Expression::Record { fields, .. } => fields.iter().for_each(|field| visit(&field.value, f)),
    }
}

/// Every element of `element`'s tree, `element` included, in document
/// order.
fn elements<'t>(element: &'t Element, out: &mut Vec<&'t Element>) {
    out.push(element);
    for child in &element.children {
        if let Child::Element { element } = child {
            elements(element, out);
        }
    }
}

/// Step 4: the assembly rules (docs/manual/templates.md), every violation
/// of every rule, in the manual's order.
fn assembly(manifest: &Manifest, root: &str, read: &[(usize, Template)]) -> Vec<RuntimeDiagnostic> {
    let mut diagnostics = Vec::new();
    let source = |template: &Template, span| Location::Source {
        component: template.component.clone(),
        span,
    };

    // Rule 1: one template per component.
    let mut first: BTreeMap<&str, &Template> = BTreeMap::new();
    for (_, template) in read {
        if first.contains_key(template.component.as_str()) {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::DUPLICATE_TEMPLATE,
                format!("a second template for `{}`", template.component),
                source(template, template.root.span),
            ));
        } else {
            first.insert(&template.component, template);
        }
    }
    // Rule 2: the root's template.
    if !first.contains_key(root) {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeCode::MISSING_ROOT,
            format!("the program has no template for its root, `{root}`"),
            Location::Program,
        ));
    }

    // Rules 4–7, for every composite occurrence in every template.
    let components = manifest.components();
    // Which composites each composite's template uses, for rule 5.
    let mut uses: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (_, template) in read {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        for element in all {
            let name = element.component.as_str();
            if !first.contains_key(name) {
                continue;
            }
            uses.entry(template.component.as_str())
                .or_default()
                .insert(name);
        }
    }
    let reaches = |from: &str, to: &str| {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                if let Some(next) = uses.get(at) {
                    stack.extend(next.iter().copied());
                }
            }
        }
        false
    };
    for (_, template) in read {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        for element in all {
            let name = element.component.as_str();
            if !first.contains_key(name) {
                continue;
            }
            let composite = &components[name];
            // Rules 4a, 4b: every scope name bound, soundly.
            for (scope_name, scope_type) in &composite.scope {
                match composite.props.get(scope_name) {
                    None => diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::UNBOUND_SCOPE_NAME,
                        format!("`{name}`'s scope name `{scope_name}` has no prop of that name to bind it"),
                        source(template, element.span),
                    )),
                    Some(prop) => {
                        let bound = if prop.required || matches!(manifest.expand(&prop.ty), Type::Optional(_)) {
                            prop.ty.clone()
                        } else {
                            Type::Optional(Box::new(prop.ty.clone()))
                        };
                        let assignable = mesh_analysis::is_assignable(
                            manifest,
                            &mesh_analysis::Ty::from(&bound),
                            &mesh_analysis::Ty::from(scope_type),
                        );
                        if !assignable {
                            diagnostics.push(RuntimeDiagnostic::new(
                                RuntimeCode::UNSOUND_BINDING,
                                format!(
                                    "`{name}`'s prop `{scope_name}` can't bind its scope name `{scope_name}`: {}",
                                    if prop.required { "its type isn't assignable" } else { "it's optional, and an unwritten prop binds absence" }
                                ),
                                source(template, element.span),
                            ));
                        }
                    }
                }
            }
            // Rule 5: no cycles.
            if reaches(name, &template.component) {
                diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::CYCLE,
                    format!(
                        "`{name}` expands `{}` again, through its template: a cycle",
                        template.component
                    ),
                    source(template, element.span),
                ));
            }
            // Rule 6: no composite events.
            if !composite.events.is_empty() {
                diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::COMPOSITE_EVENT,
                    format!("`{name}` has a template, so it's a composite, but declares events; composites can't raise events"),
                    source(template, element.span),
                ));
            }
            // Rule 7: no children.
            let has_children = element.children.iter().any(|child| match child {
                Child::Text { value, .. } => !value.trim().is_empty(),
                _ => true,
            });
            if has_children {
                diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::COMPOSITE_CHILDREN,
                    format!(
                        "`{name}` is a composite, and a composite occurrence can't have children"
                    ),
                    source(template, element.span),
                ));
            }
        }
    }
    order(&mut diagnostics);
    diagnostics
}

/// Assembly diagnostics' order: the program's first, then templates' by
/// position, then sources' by component, span start and code.
fn order(diagnostics: &mut [RuntimeDiagnostic]) {
    diagnostics.sort_by(|a, b| {
        let key = |d: &RuntimeDiagnostic| match &d.location {
            Location::Program => (0, 0, String::new(), 0),
            Location::Template { index, .. } => (1, *index, String::new(), 0),
            Location::Source { component, span } => (2, 0, component.clone(), span.start.byte),
            _ => (3, 0, String::new(), 0),
        };
        key(a).cmp(&key(b)).then_with(|| a.code.cmp(b.code))
    });
}

// --- identities -------------------------------------------------------------

/// A string, as the manuals' layouts write one: its UTF-8 length as a
/// 32-bit big-endian count, then its bytes.
fn string(hash: &mut Sha256, text: &str) {
    count(hash, text.len());
    hash.update(text.as_bytes());
}

fn count(hash: &mut Sha256, n: usize) {
    hash.update(u32::try_from(n).expect("fewer than 2^32").to_be_bytes());
}

/// The program's identity: over its root and each template's canonical
/// digest, in code-point order of component.
fn identity(program: &Program<'_>) -> [u8; 32] {
    let mut digests: Vec<(String, [u8; 32])> = program
        .templates
        .iter()
        .map(|text| {
            let value: serde_json::Value = serde_json::from_str(text).expect("validated");
            let component = value["component"].as_str().expect("validated").to_string();
            (component, canonical_digest(&value))
        })
        .collect();
    digests.sort();
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-program-v1");
    string(&mut hash, program.root);
    count(&mut hash, digests.len());
    for (component, digest) in &digests {
        string(&mut hash, component);
        hash.update(digest);
    }
    hash.finalize().into()
}

/// A template's canonical digest (docs/manual/templates.md): SHA-256 of
/// its RFC 8785 canonical JSON, as given, without `compiler`, numbers
/// written by §9.7.7.1.
pub(crate) fn canonical_digest(template: &serde_json::Value) -> [u8; 32] {
    let mut without = template.clone();
    if let Some(object) = without.as_object_mut() {
        object.remove("compiler");
    }
    let mut text = String::new();
    canonical(&without, &mut text);
    Sha256::digest(text.as_bytes()).into()
}

/// RFC 8785's canonical JSON of `value`.
pub(crate) fn canonical(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::Number(number) => {
            out.push_str(&number_to_text(
                number.as_f64().expect("a JSON number is finite"),
            ));
        }
        Value::String(text) => canonical_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            let mut members: Vec<(&String, &Value)> = members.iter().collect();
            members.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (index, (name, value)) in members.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical_string(name, out);
                out.push(':');
                canonical(value, out);
            }
            out.push('}');
        }
    }
}

fn canonical_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// One step of a structural path: a position among the parent's
/// children in the render tree, a kind, and a component.
#[derive(Debug, Clone)]
pub(crate) struct Step {
    pub position: usize,
    pub kind: u8,
    pub component: String,
}

/// A node, a composite expansion, or a text run.
pub(crate) const NODE: u8 = 0x01;
pub(crate) const TEXT: u8 = 0x02;
pub(crate) const COMPOSITE: u8 = 0x03;

/// The key at `path` in the program whose identity is `identity`.
pub(crate) fn key(identity: &[u8; 32], path: &[Step]) -> String {
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-key-v1");
    hash.update(identity);
    count(&mut hash, path.len());
    for step in path {
        count(&mut hash, step.position);
        hash.update([step.kind]);
        string(&mut hash, &step.component);
    }
    let digest: [u8; 32] = hash.finalize().into();
    format!("k{}", base64url(&digest[..16]))
}

/// The first part of every handler identifier of a program.
pub(crate) fn handler_prefix(identity: &[u8; 32]) -> String {
    format!("h{}", base64url(&identity[..8]))
}

/// The handler identifier of the handler of `event` at the node `key`.
pub(crate) fn handler(identity: &[u8; 32], key: &str, event: &str) -> String {
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-handler-v1");
    hash.update(identity);
    string(&mut hash, key);
    string(&mut hash, event);
    let digest: [u8; 32] = hash.finalize().into();
    format!("{}.{}", handler_prefix(identity), base64url(&digest[..16]))
}

/// Unpadded base64url (RFC 4648 §5).
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, byte)| n | u32::from(*byte) << (16 - 8 * i));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_is_rfc_4648s() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
        assert_eq!(base64url(&[0; 16]).len(), 22);
        assert_eq!(base64url(&[0; 8]).len(), 11);
    }

    #[test]
    fn canonical_json_is_rfc_8785s() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"b": [1, 2.50, 1e21, 0.000001, "é\n\u0001"], "a": null, "€": true, "😀": false}"#,
        )
        .unwrap();
        let mut text = String::new();
        canonical(&value, &mut text);
        // Keys by UTF-16 code units: "a", "b", "€" (U+20AC), then "😀"
        // (a surrogate pair, D83D, which sorts after 20AC).
        assert_eq!(
            text,
            "{\"a\":null,\"b\":[1,2.5,1e+21,0.000001,\"é\\n\\u0001\"],\"€\":true,\"😀\":false}"
        );
    }
}
