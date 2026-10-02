//! Render (docs/manual/runtime.md): a program, a model and a snapshot in;
//! a render out, or diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::number::number_to_text;
use crate::program::{
    self, alternatives, repeat_name, Program, Step, Valid, ALTERNATIVES, COMPOSITE, CONDITIONAL,
    CONDITIONAL_COMPONENT, NODE, REPEAT, REPEAT_COMPONENT, TEXT,
};
use crate::tree::{Node, Tree, TreeChild};
use crate::types::{fits, Statics};
use crate::value::{HostRecord, Value};
use mesh_template::{Child, Element, Expression};
use std::collections::{BTreeMap, HashSet};

/// One successful render: its tree, and the program, model and snapshot
/// it came from. It owns copies of all of them, so nothing a host does
/// afterwards reaches it, and dispatch evaluates against exactly the
/// snapshot the tree was rendered from.
#[derive(Debug, Clone)]
pub struct Render {
    pub(crate) root: String,
    pub(crate) templates: Vec<String>,
    pub(crate) model: String,
    pub(crate) snapshot: HostRecord,
    tree: Tree,
}

impl Render {
    /// The render tree, for a renderer.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub(crate) fn program(&self) -> (Vec<&str>, &str) {
        (
            self.templates.iter().map(String::as_str).collect(),
            &self.root,
        )
    }
}

/// Renders `program` against `snapshot`, validating everything first
/// (D3's phases: program validation, input validation, evaluation).
pub fn render(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let valid = program::validate(program, model)?;
    let (tree, _) = tree(&valid, snapshot, false)?;
    Ok(Render {
        root: program.root.to_string(),
        templates: program
            .templates
            .iter()
            .map(|text| (*text).to_string())
            .collect(),
        model: model.to_string(),
        snapshot: snapshot.clone(),
        tree,
    })
}

/// A handler as a render found it: the node, in the template of `component`,
/// and the **values its scope had there**, which for a repeated node include
/// the item (§9.10.7). Only a render that was asked to record has them.
pub(crate) struct Recorded<'v> {
    pub component: String,
    pub node: &'v Element,
    pub event: &'v str,
    pub values: BTreeMap<String, Value>,
}

/// Every handler of a render, by identifier.
pub(crate) type Sites<'v> = BTreeMap<String, Recorded<'v>>;

/// The render tree of a validated program, and, when `record`, the sites of
/// its handlers.
pub(crate) fn tree<'v>(
    valid: &'v Valid,
    snapshot: &HostRecord,
    record: bool,
) -> Result<(Tree, Sites<'v>), Vec<RuntimeDiagnostic>> {
    let mut inputs = Inputs::new(&valid.manifest);
    let scope = snapshot_values(valid, snapshot, &mut inputs);
    let diagnostics = inputs.sorted();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut renderer = Renderer {
        valid,
        keys: HashSet::new(),
        sites: record.then(BTreeMap::new),
    };
    let root = &valid.templates[&valid.root];
    let tree = Tree {
        root: renderer
            .occurrence(&valid.root, &root.root, 0, &mut Vec::new(), &scope)
            .map_err(|diagnostic| vec![diagnostic])?,
    };
    Ok((tree, renderer.sites.unwrap_or_default()))
}

/// The root template's scope from the snapshot (§9.8.4): each name the
/// root's component declares, validated; names it doesn't declare are
/// never looked at.
pub(crate) fn snapshot_values(
    valid: &Valid,
    snapshot: &HostRecord,
    inputs: &mut Inputs<'_>,
) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    for (name, ty) in &valid.component(&valid.root).scope {
        let mut path = vec![PathSegment::Name(name.clone())];
        let value = inputs.value(snapshot.get(name), ty, &mut path);
        values.insert(name.clone(), value);
    }
    values
}

/// An element's children as a **template** has them: maximal runs of text
/// and interpolations, elements, and conditionals. A child's position is its
/// index here: its *site's* position (spec §9.10.2), a function of the
/// template alone. A conditional is one slot whether or not it produces a
/// node, so the children after it keep their positions either way, and a
/// child's position in the render tree is not its position here.
pub(crate) enum RenderChild<'t> {
    Run(Vec<&'t Child>),
    Element(&'t Element),
    /// PROVISIONAL (§9.10 tracer): zero or one of its alternatives.
    Conditional(&'t Element),
    /// PROVISIONAL (§9.10 tracer): its child, once per item.
    Repeat(&'t Element),
}

pub(crate) fn render_children(element: &Element) -> Vec<RenderChild<'_>> {
    let mut children = Vec::new();
    let mut run: Vec<&Child> = Vec::new();
    for child in &element.children {
        match child {
            Child::Element { element } => {
                if !run.is_empty() {
                    children.push(RenderChild::Run(std::mem::take(&mut run)));
                }
                children.push(if element.component == CONDITIONAL_COMPONENT {
                    RenderChild::Conditional(element)
                } else if element.component == REPEAT_COMPONENT {
                    RenderChild::Repeat(element)
                } else {
                    RenderChild::Element(element)
                });
            }
            text_or_expression => run.push(text_or_expression),
        }
    }
    if !run.is_empty() {
        children.push(RenderChild::Run(run));
    }
    children
}

/// The scope of the template of a composite occurrence (docs/manual/
/// templates.md, "Binding a composite's scope"): each written prop,
/// evaluated in the enclosing template and checked against its declared
/// type; each scope name bound to its prop's value, or absent.
pub(crate) fn bind(
    valid: &Valid,
    scope: &Scope<'_, '_>,
    element: &Element,
) -> Result<BTreeMap<String, Value>, RuntimeDiagnostic> {
    let composite = valid.component(&element.component);
    let mut written = BTreeMap::new();
    for prop in &element.props {
        let value = scope.eval(&prop.value)?;
        let declared = &composite.props[&prop.prop].ty;
        if !fits(&valid.manifest, &value, declared) {
            return Err(scope.error(
                RuntimeCode::PROP_MISMATCH,
                format!(
                    "this value, {}, doesn't fit `{}`'s prop `{}`",
                    value.kind(),
                    element.component,
                    prop.prop
                ),
                prop.value.span(),
            ));
        }
        written.insert(prop.prop.clone(), value);
    }
    Ok(composite
        .scope
        .keys()
        .map(|name| {
            (
                name.clone(),
                written.get(name).cloned().unwrap_or(Value::Absent),
            )
        })
        .collect())
}

/// A statics context for the template of `component`.
pub(crate) fn statics<'m>(
    valid: &'m Valid,
    component: &str,
    payload: Option<&'m mesh_manifest::Type>,
) -> Statics<'m> {
    Statics {
        manifest: &valid.manifest,
        scope: &valid.component(component).scope,
        payload,
    }
}

struct Renderer<'v> {
    valid: &'v Valid,
    keys: HashSet<String>,
    sites: Option<Sites<'v>>,
}

impl<'v> Renderer<'v> {
    fn key(
        &mut self,
        path: &[Step],
        scope: &Scope<'_, '_>,
        span: mesh_template::Span,
    ) -> Result<String, RuntimeDiagnostic> {
        let key = program::key(&self.valid.identity, path);
        if !self.keys.insert(key.clone()) {
            return Err(scope.error(
                RuntimeCode::KEY_COLLISION,
                "two parts of the tree share a key",
                span,
            ));
        }
        Ok(key)
    }

    /// Renders the occurrence `element`, at `position` among its parent's
    /// render children, in the template of `component`.
    fn occurrence(
        &mut self,
        component: &str,
        element: &'v Element,
        position: usize,
        path: &mut Vec<Step>,
        values: &BTreeMap<String, Value>,
    ) -> Result<Node, RuntimeDiagnostic> {
        let scope = Scope {
            component,
            values,
            payload: None,
            statics: statics(self.valid, component, None),
        };
        if self.valid.is_composite(&element.component) {
            let bound = bind(self.valid, &scope, element)?;
            let template = &self.valid.templates[&element.component];
            path.push(Step {
                position,
                kind: COMPOSITE,
                component: element.component.clone(),
            });
            let rendered = self.occurrence(&template.component, &template.root, 0, path, &bound);
            path.pop();
            return rendered;
        }
        path.push(Step {
            position,
            kind: NODE,
            component: element.component.clone(),
        });
        let result = self.node(&scope, element, path);
        path.pop();
        result
    }

    fn node(
        &mut self,
        scope: &Scope<'_, '_>,
        element: &'v Element,
        path: &mut Vec<Step>,
    ) -> Result<Node, RuntimeDiagnostic> {
        let key = self.key(path, scope, element.span)?;
        let declared = &self.valid.component(&element.component).props;
        let mut props = BTreeMap::new();
        let mut prop_text = BTreeMap::new();
        for prop in &element.props {
            let value = scope.eval(&prop.value)?;
            let span = prop.value.span();
            if !fits(&self.valid.manifest, &value, &declared[&prop.prop].ty) {
                return Err(scope.error(
                    RuntimeCode::PROP_MISMATCH,
                    format!(
                        "this value, {}, doesn't fit `{}`'s prop `{}`",
                        value.kind(),
                        element.component,
                        prop.prop
                    ),
                    span,
                ));
            }
            if matches!(value, Value::Absent) {
                continue;
            }
            let json = output(&value).map_err(|why| scope.error(why.code, why.message, span))?;
            // The output check has passed, so a number here is finite.
            // A string's text is itself, and a list or record has none:
            // neither gets an entry.
            if matches!(value, Value::Null | Value::Boolean(_) | Value::Number(_)) {
                if let Some(text) = text_of(&value) {
                    prop_text.insert(prop.prop.clone(), text);
                }
            }
            props.insert(prop.prop.clone(), json);
        }
        let events: BTreeMap<String, String> = element
            .events
            .iter()
            .map(|binding| {
                (
                    binding.event.clone(),
                    program::handler(&self.valid.identity, &key, &binding.event),
                )
            })
            .collect();
        if let Some(sites) = &mut self.sites {
            for binding in &element.events {
                sites.insert(
                    events[&binding.event].clone(),
                    Recorded {
                        component: scope.component.to_string(),
                        node: element,
                        event: &binding.event,
                        values: scope.values.clone(),
                    },
                );
            }
        }
        let mut children = Vec::new();
        for (position, child) in render_children(element).into_iter().enumerate() {
            match child {
                RenderChild::Run(parts) => {
                    let mut text = String::new();
                    for part in parts {
                        match part {
                            Child::Text { value, .. } => text.push_str(value),
                            Child::Expression { expression } => {
                                text.push_str(&self.text(scope, expression)?)
                            }
                            Child::Element { .. } => unreachable!("a run holds no elements"),
                        }
                    }
                    path.push(Step {
                        position,
                        kind: TEXT,
                        component: String::new(),
                    });
                    let key = self.key(path, scope, element.span);
                    path.pop();
                    children.push(TreeChild::Text { key: key?, text });
                }
                RenderChild::Element(child) => {
                    children.push(TreeChild::Node(self.occurrence(
                        scope.component,
                        child,
                        position,
                        path,
                        scope.values,
                    )?));
                }
                RenderChild::Conditional(conditional) => {
                    // The slot is `position` whether or not a node comes of it.
                    if let Some((alternative, chosen)) = self.choose(scope, conditional)? {
                        path.push(Step {
                            position,
                            kind: CONDITIONAL,
                            component: ALTERNATIVES[alternative].to_string(),
                        });
                        let rendered =
                            self.occurrence(scope.component, chosen, 0, path, scope.values);
                        path.pop();
                        children.push(TreeChild::Node(rendered?));
                    }
                }
                RenderChild::Repeat(repeat) => {
                    self.repeat(scope, repeat, position, path, &mut children)?;
                }
            }
        }
        Ok(Node {
            key,
            component: element.component.clone(),
            props,
            prop_text,
            events,
            children,
        })
    }

    /// A repeat's instances (§9.10.2): its child once per item, in the
    /// order of the items, each under a step that is the item's declared key
    /// and **not** its index. The key is evaluated here, for each item, in a
    /// scope that has the item; it is a string or a finite number, and no
    /// two items of this repeat have the same one. Nothing is rendered for
    /// an item whose key isn't usable: the author's key is never replaced by
    /// a position.
    fn repeat(
        &mut self,
        scope: &Scope<'_, '_>,
        repeat: &'v Element,
        position: usize,
        path: &mut Vec<Step>,
        children: &mut Vec<TreeChild>,
    ) -> Result<(), RuntimeDiagnostic> {
        let written = |name: &str| {
            repeat
                .props
                .iter()
                .find(|prop| prop.prop == name)
                .expect("validated: a repeat has `items` and `key`")
        };
        let items = written("items");
        let key = written("key");
        let name = repeat_name(repeat).expect("validated: a repeat has an `as`");
        let value = scope.eval(&items.value)?;
        let declared = &self.valid.component(REPEAT_COMPONENT).props["items"].ty;
        let Value::List(list) = &value else {
            return Err(scope.error(
                RuntimeCode::PROP_MISMATCH,
                format!("this value, {}, isn't a list to repeat over", value.kind()),
                items.value.span(),
            ));
        };
        if !fits(&self.valid.manifest, &value, declared) {
            return Err(scope.error(
                RuntimeCode::PROP_MISMATCH,
                format!(
                    "this value, {}, doesn't fit `{REPEAT_COMPONENT}`'s prop `items`",
                    value.kind()
                ),
                items.value.span(),
            ));
        }
        let child = alternatives(repeat)[0];
        let mut seen = HashSet::new();
        for item in list.iter() {
            let mut values = scope.values.clone();
            values.insert(name.to_string(), item.clone());
            let inner = Scope {
                component: scope.component,
                values: &values,
                payload: None,
                statics: statics(self.valid, scope.component, None),
            };
            let span = key.value.span();
            let declared_key = inner.eval(&key.value)?;
            let canonical = match &declared_key {
                Value::String(text) => format!("s:{text}"),
                // `-0` and `0` are one key: §9.7.7.1's text of a number has one
                // text for both.
                Value::Number(number) if number.is_finite() => {
                    format!("n:{}", number_to_text(*number))
                }
                other => {
                    return Err(scope.error(
                        RuntimeCode::INVALID_KEY,
                        format!(
                            "this key is {}: a key is a string or a finite number",
                            if matches!(other, Value::Absent) {
                                "absent"
                            } else {
                                other.kind()
                            }
                        ),
                        span,
                    ))
                }
            };
            if !seen.insert(canonical.clone()) {
                return Err(scope.error(
                    RuntimeCode::DUPLICATE_KEY,
                    "two items of this repeat declare the same key",
                    span,
                ));
            }
            path.push(Step {
                position,
                kind: REPEAT,
                component: canonical,
            });
            let rendered = self.occurrence(scope.component, child, 0, path, &values);
            path.pop();
            children.push(TreeChild::Node(rendered?));
        }
        Ok(())
    }

    /// Which alternative of `conditional` the snapshot chooses, if any: the
    /// first when `when` is true, the second, if there is one, when it is
    /// false. `when` is checked as any prop is, and must be a boolean.
    fn choose<'t>(
        &self,
        scope: &Scope<'_, '_>,
        conditional: &'t Element,
    ) -> Result<Option<(usize, &'t Element)>, RuntimeDiagnostic> {
        let when = conditional
            .props
            .iter()
            .find(|prop| prop.prop == "when")
            .expect("validated: a conditional has a `when`");
        let span = when.value.span();
        let value = scope.eval(&when.value)?;
        let declared = &self.valid.component(CONDITIONAL_COMPONENT).props["when"].ty;
        let chosen = match value {
            Value::Boolean(true) if fits(&self.valid.manifest, &value, declared) => 0,
            Value::Boolean(false) if fits(&self.valid.manifest, &value, declared) => 1,
            _ => {
                return Err(scope.error(
                    RuntimeCode::PROP_MISMATCH,
                    format!(
                        "this value, {}, can't choose between a conditional's alternatives: `when` is a boolean",
                        value.kind()
                    ),
                    span,
                ))
            }
        };

        Ok(alternatives(conditional)
            .get(chosen)
            .map(|element| (chosen, *element)))
    }

    /// An interpolation's text (§9.7.7): the content check, then the
    /// output check, then the conversion.
    fn text(
        &self,
        scope: &Scope<'_, '_>,
        expression: &Expression,
    ) -> Result<String, RuntimeDiagnostic> {
        let span = expression.span();
        let value = scope.eval(expression)?;
        match value {
            Value::Absent => Ok(String::new()),
            Value::Number(number) if !number.is_finite() => Err(scope.error(
                RuntimeCode::NON_FINITE_OUTPUT,
                "a number that isn't finite (NaN or an infinity) has no text",
                span,
            )),
            Value::List(_) | Value::Record(_) => Err(scope.error(
                RuntimeCode::CONTENT_NOT_TEXT,
                format!("this value is {}, which has no text", value.kind()),
                span,
            )),
            _ => Ok(text_of(&value).expect("a present scalar has text")),
        }
    }
}

/// The text of a value (§9.7.7), the one conversion from a value to text,
/// used for content and for props alike: a string is itself, a boolean
/// `true` or `false`, `null` is `null`, and a finite number is §9.7.7.1's.
/// A list, a record, absence and a non-finite number have none: the
/// caller decides what that means where it is.
fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Boolean(value) => Some(value.to_string()),
        Value::String(text) => Some(text.to_string()),
        Value::Number(number) if number.is_finite() => Some(number_to_text(*number)),
        Value::Absent | Value::Number(_) | Value::List(_) | Value::Record(_) => None,
    }
}
