//! Render (docs/manual/runtime.md): a program, a model and a snapshot in;
//! a render out, or diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::number::number_to_text;
use crate::program::{self, Program, Step, Valid, COMPOSITE, NODE, TEXT};
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
    let mut inputs = Inputs::new(&valid.manifest);
    let scope = snapshot_values(&valid, snapshot, &mut inputs);
    let diagnostics = inputs.sorted();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut renderer = Renderer {
        valid: &valid,
        keys: HashSet::new(),
    };
    let root = &valid.templates[&valid.root];
    let tree = Tree {
        root: renderer
            .occurrence(&valid.root, &root.root, 0, &mut Vec::new(), &scope)
            .map_err(|diagnostic| vec![diagnostic])?,
    };
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

/// An element's children as the render tree has them: maximal runs of
/// text and interpolations, and elements. A child's position in the
/// render tree is its index here.
pub(crate) enum RenderChild<'t> {
    Run(Vec<&'t Child>),
    Element(&'t Element),
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
                children.push(RenderChild::Element(element));
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
}

impl Renderer<'_> {
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
        element: &Element,
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
        element: &Element,
        path: &mut Vec<Step>,
    ) -> Result<Node, RuntimeDiagnostic> {
        let key = self.key(path, scope, element.span)?;
        let declared = &self.valid.component(&element.component).props;
        let mut props = BTreeMap::new();
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
            let value = output(&value).map_err(|why| scope.error(why.code, why.message, span))?;
            props.insert(prop.prop.clone(), value);
        }
        let events = element
            .events
            .iter()
            .map(|binding| {
                (
                    binding.event.clone(),
                    program::handler(&self.valid.identity, &key, &binding.event),
                )
            })
            .collect();
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
            }
        }
        Ok(Node {
            key,
            component: element.component.clone(),
            props,
            events,
            children,
        })
    }

    /// An interpolation's text (§9.7.7): the content check, then the
    /// output check, then the conversion.
    fn text(
        &self,
        scope: &Scope<'_, '_>,
        expression: &Expression,
    ) -> Result<String, RuntimeDiagnostic> {
        let span = expression.span();
        Ok(match scope.eval(expression)? {
            Value::Absent => String::new(),
            Value::Null => "null".to_string(),
            Value::Boolean(value) => value.to_string(),
            Value::String(text) => text.to_string(),
            Value::Number(number) if !number.is_finite() => {
                return Err(scope.error(
                    RuntimeCode::NON_FINITE_OUTPUT,
                    "a number that isn't finite (NaN or an infinity) has no text",
                    span,
                ))
            }
            Value::Number(number) => number_to_text(number),
            value @ (Value::List(_) | Value::Record(_)) => {
                return Err(scope.error(
                    RuntimeCode::CONTENT_NOT_TEXT,
                    format!("this value is {}, which has no text", value.kind()),
                    span,
                ))
            }
        })
    }
}
