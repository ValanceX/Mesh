//! Render (docs/manual/runtime.md): a program, a model and a snapshot in;
//! a render out, or diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::number::number_to_text;
use crate::patch::Patch;
use crate::program::{
    self, alternatives, repeat_name, Program, Step, Valid, ALTERNATIVES, COMPOSITE, CONDITIONAL,
    CONDITIONAL_COMPONENT, NODE, REPEAT, REPEAT_COMPONENT, TEXT,
};
use crate::tree::{Memo, Node, RepeatItem, RepeatMemo, Run, Tree, TreeChild};
use crate::types::{fits, Statics};
use crate::value::{HostRecord, Value};
use mesh_template::{Child, Element, Expression};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

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
    let (tree, _) = tree(&valid, snapshot, false, None)?;
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

/// One update: the render of the new snapshot, and the patches that turn
/// the previous render's tree into its tree.
#[derive(Debug, Clone)]
pub struct Update {
    pub render: Render,
    pub patches: Vec<Patch>,
}

/// Renders `snapshot` in the program `previous` came from, reusing every
/// part whose inputs are unchanged, and returns the new render and the
/// patches from `previous`'s tree to its tree. The program is the previous
/// render's: a host that has a different program renders afresh. On
/// diagnostics, `previous` is untouched and still valid.
///
/// It copies `snapshot` to keep it; [`update_with`] takes it instead.
pub fn update(previous: &Render, snapshot: &HostRecord) -> Result<Update, Vec<RuntimeDiagnostic>> {
    update_with(previous, snapshot.clone())
}

/// [`update`], taking the snapshot, which the new render keeps as it is.
pub fn update_with(
    previous: &Render,
    snapshot: HostRecord,
) -> Result<Update, Vec<RuntimeDiagnostic>> {
    let templates: Vec<&str> = previous.templates.iter().map(String::as_str).collect();
    let valid = program::validate(
        &Program {
            root: &previous.root,
            templates: &templates,
        },
        &previous.model,
    )?;
    let (tree, _) = tree(&valid, &snapshot, false, Some(&previous.tree.root))?;
    let patches = crate::patch::diff(&previous.tree, &tree);
    Ok(Update {
        render: Render {
            root: previous.root.clone(),
            templates: previous.templates.clone(),
            model: previous.model.clone(),
            snapshot,
            tree,
        },
        patches,
    })
}

/// The scope names an expression reads. An expression is a pure function
/// of these (and of nothing else in a render), so a result computed from
/// the same values of them is the result.
fn reads<'t>(expression: &'t Expression, names: &mut BTreeSet<&'t str>) {
    match expression {
        Expression::Literal { .. } | Expression::Event { .. } => {}
        Expression::Scope { name, .. } => {
            names.insert(name);
        }
        Expression::Member { object, .. } => reads(object, names),
        Expression::Unary { operand, .. } => reads(operand, names),
        Expression::Binary { left, right, .. } => {
            reads(left, names);
            reads(right, names);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            reads(condition, names);
            reads(consequent, names);
            reads(alternate, names);
        }
        Expression::List { elements, .. } => elements.iter().for_each(|e| reads(e, names)),
        Expression::Record { fields, .. } => fields.iter().for_each(|f| reads(&f.value, names)),
    }
}

/// The values `names` have in `values`: absent for a name with none.
fn inputs_of(names: &BTreeSet<&str>, values: &BTreeMap<String, Value>) -> Vec<(String, Value)> {
    names
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                values.get(*name).cloned().unwrap_or(Value::Absent),
            )
        })
        .collect()
}

/// Whether `inputs` are exactly what `values` holds now.
fn unchanged(inputs: &[(String, Value)], values: &BTreeMap<String, Value>) -> bool {
    inputs.iter().all(|(name, before)| {
        values
            .get(name)
            .map_or(matches!(before, Value::Absent), |now| before.identical(now))
    })
}

/// The scope names the subtree of `element` reads from outside it (docs/
/// manual/runtime.md, "Update"): those its prop and text expressions read,
/// a conditional's `when`, a repeat's `items` and, with the repeat's own name
/// taken out, its `key` and what is under it, and a composite occurrence's
/// props (its template reads the names those props bind, in a scope of its
/// own). Event arguments aren't read: they never reach the tree.
fn free_names<'t>(
    valid: &Valid,
    element: &'t Element,
    bound: &mut Vec<&'t str>,
    out: &mut BTreeSet<&'t str>,
) {
    let mut own = BTreeSet::new();
    for prop in &element.props {
        reads(&prop.value, &mut own);
    }
    out.extend(own.into_iter().filter(|name| !bound.contains(name)));
    if element.component == REPEAT_COMPONENT {
        let name = repeat_name(element).expect("validated: a repeat has an `as`");
        bound.push(name);
        // Its `items` was read outside its own name (a name it binds can't
        // be read there); `key` and the child are read inside.
        for child in alternatives(element) {
            free_names(valid, child, bound, out);
        }
        bound.pop();
    } else if element.component == CONDITIONAL_COMPONENT {
        for alternative in alternatives(element) {
            free_names(valid, alternative, bound, out);
        }
    } else if !valid.is_composite(&element.component) {
        for child in &element.children {
            match child {
                Child::Element { element } => free_names(valid, element, bound, out),
                Child::Expression { expression } => {
                    let mut read = BTreeSet::new();
                    reads(expression, &mut read);
                    out.extend(read.into_iter().filter(|name| !bound.contains(name)));
                }
                Child::Text { .. } => {}
            }
        }
    }
}

/// The nodes of the previous render's tree that can be this render's, by key:
/// the children of the node being rendered (or, for the root, the root).
type Old<'o> = HashMap<&'o str, &'o Rc<Node>>;

fn old_children(node: Option<&Rc<Node>>) -> Old<'_> {
    node.map(|node| {
        node.children
            .iter()
            .filter_map(|child| match child {
                TreeChild::Node(child) => Some((child.key.as_str(), child)),
                TreeChild::Text { .. } => None,
            })
            .collect()
    })
    .unwrap_or_default()
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
    previous: Option<&Rc<Node>>,
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
        free: HashMap::new(),
        prefixes: Vec::new(),
    };
    let root = &valid.templates[&valid.root];
    let old: Old<'_> = previous
        .map(|root| HashMap::from([(root.key.as_str(), root)]))
        .unwrap_or_default();
    let tree = Tree {
        root: renderer
            .occurrence(&valid.root, &root.root, 0, &mut Vec::new(), &scope, &old)
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
    /// The scope names read from outside each element's subtree, by element.
    free: HashMap<*const Element, Rc<Vec<String>>>,
    /// For the repeats being rendered, innermost last: the hash of the path
    /// above an item's node, finished for each item (total path length, the
    /// number of steps the hash holds, and the hash).
    prefixes: Vec<(usize, usize, program::KeyPrefix)>,
}

impl<'v> Renderer<'v> {
    /// The scope names the subtree of `element` reads from outside it.
    fn free(&mut self, element: &'v Element) -> Rc<Vec<String>> {
        let valid = self.valid;
        Rc::clone(self.free.entry(element).or_insert_with(|| {
            let mut names = BTreeSet::new();
            free_names(valid, element, &mut Vec::new(), &mut names);
            Rc::new(names.into_iter().map(str::to_string).collect())
        }))
    }

    /// The key at `path`, finished from the innermost repeat's shared hash
    /// when `path` is as long as the paths it was made for.
    fn peek(&self, path: &[Step]) -> String {
        match self.prefixes.last() {
            Some((total, hashed, prefix)) if *total == path.len() => prefix.key(&path[*hashed..]),
            _ => program::key(&self.valid.identity, path),
        }
    }

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
        old: &Old<'_>,
    ) -> Result<Rc<Node>, RuntimeDiagnostic> {
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
            let rendered =
                self.occurrence(&template.component, &template.root, 0, path, &bound, old);
            path.pop();
            return rendered;
        }
        path.push(Step {
            position,
            kind: NODE,
            component: element.component.clone(),
        });
        let result = self.node(&scope, element, path, old);
        path.pop();
        result
    }

    fn node(
        &mut self,
        scope: &Scope<'_, '_>,
        element: &'v Element,
        path: &mut Vec<Step>,
        old: &Old<'_>,
    ) -> Result<Rc<Node>, RuntimeDiagnostic> {
        let key = self.peek(path);
        let previous = old.get(key.as_str()).copied();
        // A subtree whose inputs are all as they were is the same subtree:
        // keep it, without looking inside.
        let free = self.free(element);
        if let Some(previous) = previous {
            if unchanged(&previous.memo.free, scope.values)
                && previous.memo.free.len() == free.len()
            {
                return Ok(Rc::clone(previous));
            }
        }
        // A part that is rendered claims its key; a part kept doesn't
        // need to, since the tree it is kept from is already known to have it once.
        if !self.keys.insert(key.clone()) {
            return Err(scope.error(
                RuntimeCode::KEY_COLLISION,
                "two parts of the tree share a key",
                element.span,
            ));
        }
        let declared = &self.valid.component(&element.component).props;
        let mut props = BTreeMap::new();
        let mut prop_text = BTreeMap::new();
        let mut read = BTreeSet::new();
        element
            .props
            .iter()
            .for_each(|prop| reads(&prop.value, &mut read));
        let reused = previous.filter(|previous| unchanged(&previous.memo.own, scope.values));
        let computed = reused.is_none();
        if let Some(previous) = reused {
            props = previous.props.clone();
            prop_text = previous.prop_text.clone();
        }
        for prop in element.props.iter().filter(|_| computed) {
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
        let old_below = old_children(previous);
        let mut runs: Vec<Run> = Vec::new();
        let mut repeats: Vec<RepeatMemo> = Vec::new();
        let mut children = Vec::new();
        for (position, child) in render_children(element).into_iter().enumerate() {
            match child {
                RenderChild::Run(parts) => {
                    path.push(Step {
                        position,
                        kind: TEXT,
                        component: String::new(),
                    });
                    let peeked = program::key(&self.valid.identity, path);
                    path.pop();
                    let mut read = BTreeSet::new();
                    for part in &parts {
                        if let Child::Expression { expression } = part {
                            reads(expression, &mut read);
                        }
                    }
                    let reused = previous
                        .and_then(|previous| {
                            previous.memo.runs.iter().find(|(key, ..)| *key == peeked)
                        })
                        .filter(|(_, inputs, _)| unchanged(inputs, scope.values))
                        .map(|(_, _, text)| text.clone());
                    let text = match reused {
                        Some(text) => text,
                        None => {
                            let mut text = String::new();
                            for part in parts {
                                match part {
                                    Child::Text { value, .. } => text.push_str(value),
                                    Child::Expression { expression } => {
                                        text.push_str(&self.text(scope, expression)?)
                                    }
                                    Child::Element { .. } => {
                                        unreachable!("a run holds no elements")
                                    }
                                }
                            }
                            text
                        }
                    };
                    path.push(Step {
                        position,
                        kind: TEXT,
                        component: String::new(),
                    });
                    let key = self.key(path, scope, element.span);
                    path.pop();
                    let key = key?;
                    runs.push((key.clone(), inputs_of(&read, scope.values), text.clone()));
                    children.push(TreeChild::Text { key, text });
                }
                RenderChild::Element(child) => {
                    children.push(TreeChild::Node(self.occurrence(
                        scope.component,
                        child,
                        position,
                        path,
                        scope.values,
                        &old_below,
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
                        let rendered = self.occurrence(
                            scope.component,
                            chosen,
                            0,
                            path,
                            scope.values,
                            &old_below,
                        );
                        path.pop();
                        children.push(TreeChild::Node(rendered?));
                    }
                }
                RenderChild::Repeat(repeat) => {
                    repeats.push(self.repeat(
                        scope,
                        repeat,
                        position,
                        path,
                        &mut children,
                        &old_below,
                        previous,
                    )?);
                }
            }
        }
        Ok(Rc::new(Node {
            key,
            component: element.component.clone(),
            props,
            prop_text,
            events,
            children,
            memo: Memo {
                free: inputs_of(&free.iter().map(String::as_str).collect(), scope.values),
                own: inputs_of(&read, scope.values),
                runs,
                repeats,
            },
        }))
    }

    /// A repeat's instances (§9.10.2): its child once per item, in the
    /// order of the items, each under a step that is the item's declared key
    /// and **not** its index. The key is evaluated here, for each item, in a
    /// scope that has the item; it is a string or a finite number, and no
    /// two items of this repeat have the same one. Nothing is rendered for
    /// an item whose key isn't usable: the author's key is never replaced by
    /// a position.
    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &mut self,
        scope: &Scope<'_, '_>,
        repeat: &'v Element,
        position: usize,
        path: &mut Vec<Step>,
        children: &mut Vec<TreeChild>,
        old: &Old<'_>,
        previous: Option<&Rc<Node>>,
    ) -> Result<RepeatMemo, RuntimeDiagnostic> {
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
        let mut seen: HashSet<Rc<str>> = HashSet::new();
        // What the key reads besides the item: if it is as it was, an item
        // that is as it was has the key it had.
        let mut key_reads = BTreeSet::new();
        reads(&key.value, &mut key_reads);
        key_reads.remove(name);
        let key_other = inputs_of(&key_reads, scope.values);
        let before = previous
            .and_then(|previous| {
                previous
                    .memo
                    .repeats
                    .iter()
                    .find(|memo| memo.position == position)
            })
            .filter(|memo| unchanged(&memo.key_other, scope.values));
        let mut made: Vec<RepeatItem> = Vec::with_capacity(list.len());
        // One scope for the whole repeat, whose slot for the item is
        // overwritten for each item: an item's scope is the repeat's with the
        // item in it, and a map per item would be an allocation per name.
        let mut values = scope.values.clone();
        values.insert(name.to_string(), Value::Absent);
        // An item's node is two steps below the repeat (its site, then the
        // child), so its path is that long: every item's key starts from one hash.
        self.prefixes.push((
            path.len() + 2,
            path.len(),
            program::KeyPrefix::new(&self.valid.identity, path.len() + 2, path),
        ));
        for (index, item) in list.iter().enumerate() {
            *values.get_mut(name).expect("the slot was made above") = item.clone();
            // The item at this place before, if it is the same item and what
            // its node was made from is as it was, is the item's node again.
            if let Some(kept) = before
                .and_then(|memo| memo.items.get(index))
                .filter(|kept| {
                    kept.item.identical(item) && unchanged(&kept.node.memo.free, &values)
                })
            {
                if !seen.insert(Rc::clone(&kept.canonical)) {
                    return Err(scope.error(
                        RuntimeCode::DUPLICATE_KEY,
                        "two items of this repeat declare the same key",
                        key.value.span(),
                    ));
                }
                children.push(TreeChild::Node(Rc::clone(&kept.node)));
                made.push(kept.clone());
                continue;
            }
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
            let canonical: Rc<str> = Rc::from(canonical);
            if !seen.insert(Rc::clone(&canonical)) {
                return Err(scope.error(
                    RuntimeCode::DUPLICATE_KEY,
                    "two items of this repeat declare the same key",
                    span,
                ));
            }
            path.push(Step {
                position,
                kind: REPEAT,
                component: canonical.to_string(),
            });
            let rendered = self.occurrence(scope.component, child, 0, path, &values, old);
            path.pop();
            let rendered = rendered?;
            made.push(RepeatItem {
                canonical,
                item: item.clone(),
                node: Rc::clone(&rendered),
            });
            children.push(TreeChild::Node(rendered));
        }
        self.prefixes.pop();
        Ok(RepeatMemo {
            position,
            key_other,
            items: made,
        })
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

#[cfg(test)]
mod timing {
    use super::*;
    use std::time::Instant;

    /// Not a test of anything: what the parts of one repeated item cost.
    #[test]
    #[ignore = "a measurement: prints, asserts nothing"]
    fn what_one_item_costs() {
        let identity = [7u8; 32];
        let path: Vec<Step> = (0..4)
            .map(|i| Step {
                position: i,
                kind: NODE,
                component: "page".to_string(),
            })
            .collect();
        let n = 20_000;
        let t = Instant::now();
        for _ in 0..n {
            std::hint::black_box(program::key(&identity, &path));
        }
        println!("key (sha-256, depth 4): {:?} each", t.elapsed() / n);
        let t = Instant::now();
        for i in 0..n {
            std::hint::black_box(number_to_text(f64::from(i)));
        }
        println!("number_to_text: {:?} each", t.elapsed() / n);
        let t = Instant::now();
        for i in 0..n {
            std::hint::black_box(format!("n:{}", number_to_text(f64::from(i))));
        }
        println!("canonical key text: {:?} each", t.elapsed() / n);
    }
}
