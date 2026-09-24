//! Checks a parsed manifest document against the version 1 format and
//! the model's own rules, and builds the [`Manifest`].
//!
//! The loader's phases (phase 1, JSON syntax, runs in `json.rs`; phase 6,
//! template selection, is [`Manifest::template`](crate::Manifest::template)):
//! 2. **Version gate.** A root that isn't an object, or a missing or
//!    unsupported `"version"`, is the only error reported.
//! 3. **Structure:** JSON types, required and unknown properties,
//!    repeated keys, kinds, and an `optional` directly wrapping an
//!    `optional`.
//! 4. **Declarations:** name rules, and repeated parameter names.
//! 5. **Named-type graph**, over every declared named type: unknown
//!    references, recursion, and an `optional` wrapping a named type that
//!    expands to `optional`.
//!
//! Phases 3 and 4 run as one traversal of the document; phase 5 runs
//! after it. Every problem they find is reported. A value that is wrong is
//! reported once and nothing inside it is checked, so one mistake doesn't
//! cascade; its siblings are still checked. Diagnostics are sorted by
//! start byte, and ties keep emission order (the traversal before phase
//! 5, document order within each). The [`Manifest`] is built only if
//! nothing was reported.

use crate::json::{Json, Member, Value};
use crate::{Command, Component, Event, Field, Manifest, Parameter, Type, VERSION};
use mesh_syntax::{Diagnostic, DiagnosticCode, Severity, Span};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(crate) fn manifest(document: &Json) -> Result<Manifest, Vec<Diagnostic>> {
    let mut validator = Validator::default();
    let manifest = validator.document(document);
    if validator.diagnostics.is_empty() {
        Ok(manifest.expect("a manifest with no diagnostics was built"))
    } else {
        // Checks run section by section; report in source order.
        validator.diagnostics.sort_by_key(|d| d.span.start_byte);
        Err(validator.diagnostics)
    }
}

/// What a declared name must look like.
#[derive(Clone, Copy)]
enum NameRule {
    /// A component: an MPRX tag name, such as `user-card`.
    Tag,
    /// Anything else: an MPRX identifier, such as `selectUser`.
    Identifier,
}

/// Words MPRX reads as literals, so no identifier can be one.
const RESERVED: [&str; 3] = ["true", "false", "null"];

/// A `named` type reference, checked once every named type is known.
struct NamedRef {
    name: String,
    /// The reference's `"name"` value.
    span: Span,
    /// The named type whose definition contains the reference, if any.
    in_definition: Option<String>,
    /// Whether an `optional` type directly wraps the reference.
    wrapped_in_optional: bool,
}

#[derive(Default)]
struct Validator {
    diagnostics: Vec<Diagnostic>,
    /// Every named type declared, first occurrence only, in source order,
    /// with its key's span. Declared means a reference to it isn't
    /// unknown, even if its definition has errors.
    declared_types: Vec<(String, Span)>,
    /// The named types whose definitions are valid.
    types: BTreeMap<String, Type>,
    refs: Vec<NamedRef>,
    /// The named type being defined while its definition is checked.
    in_definition: Option<String>,
}

/// An object's properties by key, first occurrence only.
struct Properties<'j> {
    members: Vec<&'j Member>,
}

impl<'j> Properties<'j> {
    fn get(&self, key: &str) -> Option<&'j Json> {
        self.members
            .iter()
            .find(|member| member.key == key)
            .map(|member| &member.value)
    }
}

/// The first byte of a value: an object's `{`. A missing property is
/// reported here rather than across the whole object.
fn opening(json: &Json) -> Span {
    Span {
        start_byte: json.span.start_byte,
        end_byte: json.span.start_byte + 1,
    }
}

impl Validator {
    fn error(&mut self, code: DiagnosticCode, span: Span, message: String) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            message,
            span,
        });
    }

    fn document(&mut self, json: &Json) -> Option<Manifest> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "a manifest must be a JSON object, found {}",
                    json.value.describe()
                ),
            );
            return None;
        };

        // A manifest of another version has another schema, so nothing
        // else in it can be checked.
        let Some(version) = members.iter().find(|member| member.key == "version") else {
            self.error(
                DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                opening(json),
                format!("the manifest has no \"version\"; this MESH reads version {VERSION}"),
            );
            return None;
        };
        match &version.value.value {
            Value::Number(number) if number.as_u64() == Some(VERSION) => {}
            Value::Number(number) => {
                self.error(
                    DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                    version.value.span,
                    format!(
                        "unsupported manifest version {number}; this MESH reads version {VERSION}"
                    ),
                );
                return None;
            }
            other => {
                self.error(
                    DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
                    version.value.span,
                    format!(
                        "the manifest version must be the number {VERSION}, found {}",
                        other.describe()
                    ),
                );
                return None;
            }
        }

        let properties = self.object(
            json,
            "the manifest",
            &["version", "types", "components"],
            &["$schema"],
        )?;
        if let Some(schema) = properties.get("$schema") {
            self.string(schema, "\"$schema\"");
        }

        if let Some(types) = properties.get("types") {
            for member in self.names(
                types,
                "types",
                "type",
                "the manifest's \"types\"",
                NameRule::Identifier,
            ) {
                self.declared_types
                    .push((member.key.clone(), member.key_span));
                self.in_definition = Some(member.key.clone());
                if let Some(ty) = self.ty(&member.value) {
                    self.types.insert(member.key.clone(), ty);
                }
                self.in_definition = None;
            }
        }

        let mut components = Some(BTreeMap::new());
        if let Some(json) = properties.get("components") {
            for member in self.names(
                json,
                "components",
                "component",
                "\"components\"",
                NameRule::Tag,
            ) {
                let component = self.component(&member.key, &member.value);
                insert(&mut components, &member.key, component);
            }
        }

        self.check_named_types();

        let components_span = members
            .iter()
            .find(|member| member.key == "components")
            .map(|member| member.key_span)?;
        Some(Manifest {
            types: std::mem::take(&mut self.types),
            components: components?,
            components_span,
        })
    }

    fn component(&mut self, name: &str, json: &Json) -> Option<Component> {
        let what = format!("component {name:?}");
        let properties =
            self.object(json, &what, &["props", "events", "commands", "scope"], &[])?;
        let owner = format!("component {name:?}");

        let mut props = Some(BTreeMap::new());
        if let Some(json) = properties.get("props") {
            for member in self.names(json, "props", "prop", &owner, NameRule::Identifier) {
                let field = self.field(&member.value, &format!("prop {:?}", member.key));
                insert(&mut props, &member.key, field);
            }
        }

        let mut events = Some(BTreeMap::new());
        if let Some(json) = properties.get("events") {
            for member in self.names(json, "events", "event", &owner, NameRule::Identifier) {
                let event = self.event(&member.value, &member.key);
                insert(&mut events, &member.key, event);
            }
        }

        let mut commands = Some(BTreeMap::new());
        if let Some(json) = properties.get("commands") {
            for member in self.names(json, "commands", "command", &owner, NameRule::Identifier) {
                let command = self.command(&member.value, &member.key);
                insert(&mut commands, &member.key, command);
            }
        }

        let mut scope = Some(BTreeMap::new());
        if let Some(json) = properties.get("scope") {
            for member in self.names(json, "scope", "scope name", &owner, NameRule::Identifier) {
                let ty = self.ty(&member.value);
                insert(&mut scope, &member.key, ty);
            }
        }

        Some(Component {
            props: props?,
            events: events?,
            commands: commands?,
            scope: scope?,
        })
    }

    fn event(&mut self, json: &Json, name: &str) -> Option<Event> {
        let properties = self.object(json, &format!("event {name:?}"), &[], &["payload"])?;
        let payload = match properties.get("payload") {
            Some(json) => Some(self.ty(json)?),
            None => None,
        };
        Some(Event { payload })
    }

    fn command(&mut self, json: &Json, name: &str) -> Option<Command> {
        let properties = self.object(json, &format!("command {name:?}"), &["parameters"], &[])?;
        let json = properties.get("parameters")?;
        let Value::Array(items) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "\"parameters\" must be an array, found {}",
                    json.value.describe()
                ),
            );
            return None;
        };

        let mut parameters = Some(Vec::new());
        let mut seen = HashSet::new();
        for item in items {
            let parameter = self.parameter(item, name, &mut seen);
            match (&mut parameters, parameter) {
                (Some(parameters), Some(parameter)) => parameters.push(parameter),
                _ => parameters = None,
            }
        }
        Some(Command {
            parameters: parameters?,
        })
    }

    fn parameter(
        &mut self,
        json: &Json,
        command: &str,
        seen: &mut HashSet<String>,
    ) -> Option<Parameter> {
        let what = format!("a parameter of command {command:?}");
        let properties = self.object(json, &what, &["name", "type"], &[])?;
        let name_json = properties.get("name");
        let name = name_json.and_then(|json| self.string(json, "a parameter's \"name\""));
        if let (Some(name), Some(json)) = (&name, name_json) {
            if self.valid_name(name, json.span, "parameter", NameRule::Identifier)
                && !seen.insert(name.clone())
            {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_PARAMETER,
                    json.span,
                    format!("command {command:?} has two parameters named {name:?}"),
                );
            }
        }
        let ty = properties.get("type").and_then(|json| self.ty(json));
        Some(Parameter {
            name: name?,
            ty: ty?,
        })
    }

    /// A prop or record field declaration: `{ "type": ..., "required": ... }`.
    fn field(&mut self, json: &Json, what: &str) -> Option<Field> {
        let properties = self.object(json, what, &["type", "required"], &[])?;
        let ty = properties.get("type").and_then(|json| self.ty(json));
        let required = properties
            .get("required")
            .and_then(|json| match json.value {
                Value::Bool(required) => Some(required),
                ref other => {
                    self.error(
                        DiagnosticCode::MANIFEST_INVALID_VALUE,
                        json.span,
                        format!(
                            "\"required\" must be true or false, found {}",
                            other.describe()
                        ),
                    );
                    None
                }
            });
        Some(Field {
            ty: ty?,
            required: required?,
        })
    }

    fn ty(&mut self, json: &Json) -> Option<Type> {
        self.ty_in(json, false)
    }

    /// A type. `in_optional` is whether an `optional` type directly wraps
    /// this one.
    fn ty_in(&mut self, json: &Json, in_optional: bool) -> Option<Type> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "a type must be an object with a \"kind\", found {}",
                    json.value.describe()
                ),
            );
            return None;
        };
        let Some(kind) = members.iter().find(|member| member.key == "kind") else {
            self.error(
                DiagnosticCode::MANIFEST_MISSING_PROPERTY,
                opening(json),
                "a type must have a \"kind\"".to_string(),
            );
            return None;
        };
        let Value::String(kind_name) = &kind.value.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                kind.value.span,
                format!(
                    "\"kind\" must be a string, found {}",
                    kind.value.value.describe()
                ),
            );
            self.repeated_kinds(members);
            return None;
        };

        let primitive = match kind_name.as_str() {
            "string" => Some(Type::String),
            "number" => Some(Type::Number),
            "boolean" => Some(Type::Boolean),
            "null" => Some(Type::Null),
            "any" => Some(Type::Any),
            _ => None,
        };
        if let Some(primitive) = primitive {
            self.object(json, &format!("a {kind_name} type"), &["kind"], &[])?;
            return Some(primitive);
        }

        match kind_name.as_str() {
            "list" => {
                let properties = self.object(json, "a list type", &["kind", "element"], &[])?;
                let element = self.ty(properties.get("element")?)?;
                Some(Type::List(Box::new(element)))
            }
            "record" => {
                let properties = self.object(json, "a record type", &["kind", "fields"], &[])?;
                let fields_json = properties.get("fields")?;
                let mut fields = Some(BTreeMap::new());
                for member in self.names(
                    fields_json,
                    "fields",
                    "field",
                    "a record type",
                    NameRule::Identifier,
                ) {
                    let field = self.field(&member.value, &format!("field {:?}", member.key));
                    insert(&mut fields, &member.key, field);
                }
                Some(Type::Record(fields?))
            }
            "named" => {
                let properties = self.object(json, "a named type", &["kind", "name"], &[])?;
                let name_json = properties.get("name")?;
                let name = self.string(name_json, "a named type's \"name\"")?;
                self.refs.push(NamedRef {
                    name: name.clone(),
                    span: name_json.span,
                    in_definition: self.in_definition.clone(),
                    wrapped_in_optional: in_optional,
                });
                Some(Type::Named(name))
            }
            "optional" => {
                let properties = self.object(json, "an optional type", &["kind", "type"], &[])?;
                let inner_json = properties.get("type")?;
                let inner = self.ty_in(inner_json, true)?;
                if let Type::Optional(_) = inner {
                    self.error(
                        DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
                        inner_json.span,
                        "an optional type can't wrap another optional type".to_string(),
                    );
                    return None;
                }
                Some(Type::Optional(Box::new(inner)))
            }
            "void" | "nothing" => {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_KIND,
                    kind.value.span,
                    format!("`{kind_name}` is internal to MESH; a manifest can't write it"),
                );
                self.repeated_kinds(members);
                None
            }
            _ => {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_KIND,
                    kind.value.span,
                    format!(
                        "unknown type kind {kind_name:?}; expected string, number, boolean, \
                         null, any, list, record, named or optional"
                    ),
                );
                self.repeated_kinds(members);
                None
            }
        }
    }

    /// Reports every `"kind"` after the first in a type object whose
    /// first `"kind"` is invalid. Such an object's other properties aren't
    /// checked, but a repeated `"kind"` is still reported, so it can't
    /// look as though the later one was used. (With a valid first
    /// `"kind"`, [`Validator::object`] reports repeats.)
    fn repeated_kinds(&mut self, members: &[Member]) {
        for member in members.iter().filter(|member| member.key == "kind").skip(1) {
            self.error(
                DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                member.key_span,
                "a type repeats the property \"kind\"".to_string(),
            );
        }
    }

    /// Checks that `json` is an object with every `required` property,
    /// no property outside `required` and `optional`, and no repeated key.
    /// Returns `None` only if `json` isn't an object at all.
    fn object<'j>(
        &mut self,
        json: &'j Json,
        what: &str,
        required: &[&str],
        optional: &[&str],
    ) -> Option<Properties<'j>> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!("{what} must be an object, found {}", json.value.describe()),
            );
            return None;
        };

        let mut first = Vec::new();
        let mut seen = HashSet::new();
        for member in members {
            if !seen.insert(member.key.as_str()) {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                    member.key_span,
                    format!("{what} repeats the property {:?}", member.key),
                );
            } else if required.contains(&member.key.as_str())
                || optional.contains(&member.key.as_str())
            {
                first.push(member);
            } else {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY,
                    member.key_span,
                    format!("{what} can't have the property {:?}", member.key),
                );
            }
        }
        for property in required {
            if !seen.contains(property) {
                self.error(
                    DiagnosticCode::MANIFEST_MISSING_PROPERTY,
                    opening(json),
                    format!("{what} is missing the property {property:?}"),
                );
            }
        }
        Some(Properties { members: first })
    }

    /// Checks that `json`, the value of `property`, is an object mapping
    /// declared names to declarations: each name valid under `rule`, and
    /// none repeated.
    /// Returns the members to check further: the first occurrence of each
    /// name.
    fn names<'j>(
        &mut self,
        json: &'j Json,
        property: &str,
        noun: &str,
        owner: &str,
        rule: NameRule,
    ) -> Vec<&'j Member> {
        let Value::Object(members) = &json.value else {
            self.error(
                DiagnosticCode::MANIFEST_INVALID_VALUE,
                json.span,
                format!(
                    "{property:?} must be an object, found {}",
                    json.value.describe()
                ),
            );
            return Vec::new();
        };

        let mut first = Vec::new();
        let mut seen = HashSet::new();
        for member in members {
            if !seen.insert(member.key.as_str()) {
                self.error(
                    DiagnosticCode::MANIFEST_DUPLICATE_KEY,
                    member.key_span,
                    format!("{owner} declares the {noun} {:?} twice", member.key),
                );
                continue;
            }
            self.valid_name(&member.key, member.key_span, noun, rule);
            first.push(member);
        }
        first
    }

    /// Reports `name` if MPRX can't write it where a `noun` goes.
    fn valid_name(&mut self, name: &str, span: Span, noun: &str, rule: NameRule) -> bool {
        let mut chars = name.chars();
        let starts_well = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        let rest_ok = chars.all(|c| match rule {
            NameRule::Tag => c.is_ascii_alphanumeric() || c == '_' || c == '-',
            NameRule::Identifier => c.is_ascii_alphanumeric() || c == '_',
        });
        let message = if !(starts_well && rest_ok) {
            match rule {
                NameRule::Tag => format!(
                    "{noun} name {name:?} isn't a valid MPRX tag name: start with a letter \
                     or `_`, then use letters, digits, `_` or `-`"
                ),
                NameRule::Identifier => format!(
                    "{noun} name {name:?} isn't a valid MPRX identifier: start with a letter \
                     or `_`, then use letters, digits or `_`"
                ),
            }
        } else if matches!(rule, NameRule::Identifier) && RESERVED.contains(&name) {
            format!("{noun} name {name:?} is reserved: MPRX reads it as a literal")
        } else {
            return true;
        };
        self.error(DiagnosticCode::MANIFEST_INVALID_NAME, span, message);
        false
    }

    fn string(&mut self, json: &Json, what: &str) -> Option<String> {
        match &json.value {
            Value::String(value) => Some(value.clone()),
            other => {
                self.error(
                    DiagnosticCode::MANIFEST_INVALID_VALUE,
                    json.span,
                    format!("{what} must be a string, found {}", other.describe()),
                );
                None
            }
        }
    }

    /// The checks that need every named type: references resolve, no
    /// optional wraps a named type that is already optional, and no named
    /// type refers to itself.
    fn check_named_types(&mut self) {
        let declared: BTreeSet<String> = self
            .declared_types
            .iter()
            .map(|(name, _)| name.clone())
            .collect();

        let refs = std::mem::take(&mut self.refs);

        // Each cycle is reported once, at the first of its types in the
        // source.
        let mut recursive = HashSet::new();
        for (name, key_span) in self.declared_types.clone() {
            if recursive.contains(&name) {
                continue;
            }
            if let Some(cycle) = find_cycle(&name, &refs) {
                self.error(
                    DiagnosticCode::MANIFEST_RECURSIVE_TYPE,
                    key_span,
                    format!("type {name:?} refers to itself: {}", cycle.join(" -> ")),
                );
                recursive.extend(cycle);
            }
        }

        for reference in &refs {
            if !declared.contains(&reference.name) {
                self.error(
                    DiagnosticCode::MANIFEST_UNKNOWN_TYPE,
                    reference.span,
                    format!(
                        "unknown type {:?}: the manifest's \"types\" doesn't declare it",
                        reference.name
                    ),
                );
            } else if reference.wrapped_in_optional
                // A recursive type has been reported already.
                && !recursive.contains(&reference.name)
                && self.is_optional(&reference.name)
            {
                self.error(
                    DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
                    reference.span,
                    format!(
                        "an optional type can't wrap {:?}, which is already optional",
                        reference.name
                    ),
                );
            }
        }
    }

    /// Whether named type `name` is, after following named references,
    /// an optional type. An unknown or invalid definition counts as not
    /// optional: it has been reported already.
    fn is_optional(&self, name: &str) -> bool {
        let mut seen = HashSet::new();
        let mut ty = self.types.get(name);
        while let Some(current) = ty {
            match current {
                Type::Optional(_) => return true,
                Type::Named(next) if seen.insert(next.as_str()) => ty = self.types.get(next),
                _ => return false,
            }
        }
        false
    }
}

/// A path of named-type references from `start` back to `start`, such as
/// `["Node", "Children", "Node"]`, if there is one.
fn find_cycle(start: &str, refs: &[NamedRef]) -> Option<Vec<String>> {
    fn visit(
        current: &str,
        start: &str,
        refs: &[NamedRef],
        path: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) -> bool {
        for reference in refs {
            if reference.in_definition.as_deref() != Some(current) {
                continue;
            }
            if reference.name == start {
                path.push(start.to_string());
                return true;
            }
            if visited.insert(reference.name.clone()) {
                path.push(reference.name.clone());
                if visit(&reference.name, start, refs, path, visited) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }

    let mut path = vec![start.to_string()];
    let mut visited = HashSet::new();
    visit(start, start, refs, &mut path, &mut visited).then_some(path)
}

/// Adds a checked declaration to a map being built. A declaration with
/// errors (`None`) poisons the whole map, since the manifest won't be
/// built anyway.
fn insert<T>(map: &mut Option<BTreeMap<String, T>>, name: &str, value: Option<T>) {
    match (map.as_mut(), value) {
        (Some(map), Some(value)) => {
            map.insert(name.to_string(), value);
        }
        _ => *map = None,
    }
}
