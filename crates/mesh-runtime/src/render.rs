//! Render (docs/manual/runtime.md): a program, a model and a snapshot in;
//! a render out, or diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::number::number_to_text;
use crate::patch::Patch;
use crate::program::{
    self, alternatives, repeat_name, Program, Step, Valid, ALTERNATIVES, COMPOSITE, CONDITIONAL,
    CONDITIONAL_COMPONENT, FILL_COMPONENT, FRAGMENT, FRAGMENT_COMPONENT, NODE, REPEAT,
    REPEAT_COMPONENT, SLOT, SLOT_COMPONENT, TEXT,
};
use crate::tree::{Memo, Node, RepeatItem, RepeatMemo, Run, Tree, TreeChild};
use crate::types::{fits, Statics};
use crate::value::{HostKey, HostRecord, HostValue, Value};
use mesh_template as mpl;
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
    /// The root scope's values, validated: the snapshot as the render
    /// has it. Shared (`Rc`) with the renders made from it by changes.
    pub(crate) values: BTreeMap<String, Value>,
    tree: Tree,
    /// This render's version in this thread: no two renders have the same
    /// one. Changes name the version they were computed against.
    id: u64,
}

thread_local! {
    static VERSIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The next render's version: 1, 2, 3, ... never repeated in a thread.
pub(crate) fn next_version() -> u64 {
    VERSIONS.with(|count| {
        count.set(count.get() + 1);
        count.get()
    })
}

impl Render {
    /// The render tree, for a renderer.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// This render's version: unique among the renders made in this thread.
    /// Changes to apply to it (`update_changes`) name it as their `base`, so
    /// that they can't be applied to any other render.
    pub fn version(&self) -> u64 {
        self.id
    }

    /// This render under `version` instead of the one it was given. For a host
    /// that derives again a render it has already named by its version (the
    /// JavaScript package does, for changes to a render the module no longer
    /// holds), so that changes naming that version still apply to it. Two
    /// renders may then share a version; changes are applied to the one the
    /// caller passes, so the version only has to match that render's.
    pub fn with_version(mut self, version: u64) -> Render {
        self.id = version;
        self
    }

    /// The snapshot this render has, as a host would give it: for the dispatch
    /// that validates a render's inputs again.
    pub(crate) fn host_snapshot(&self) -> HostRecord {
        HostRecord(
            self.values
                .iter()
                .filter(|(_, value)| !matches!(value, Value::Absent))
                .map(|(name, value)| (HostKey::Text(name.clone()), to_host(value)))
                .collect(),
        )
    }

    pub(crate) fn program(&self) -> (Vec<&str>, &str) {
        (
            self.templates.iter().map(String::as_str).collect(),
            &self.root,
        )
    }
}

/// A value as a host would give it: the inverse of input validation, for
/// values that passed it.
fn to_host(value: &Value) -> HostValue {
    match value {
        Value::Absent => unreachable!("a record holds no absent field; a list's are refused"),
        Value::Null => HostValue::Null,
        Value::Boolean(value) => HostValue::Boolean(*value),
        Value::Number(number) => HostValue::Number(*number),
        Value::String(text) => HostValue::String(text.to_string()),
        Value::List(items) => HostValue::List(
            items
                .iter()
                .map(|item| match item {
                    Value::Absent => None,
                    item => Some(to_host(item)),
                })
                .collect(),
        ),
        Value::Record(fields) => HostValue::Record(HostRecord(
            fields
                .iter()
                .map(|(name, value)| (HostKey::Text(name.clone()), to_host(value)))
                .collect(),
        )),
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
    let values = validated_values(&valid, snapshot)?;
    let (tree, _) = tree_of(&valid, &values, false, None)?;
    Ok(Render {
        root: program.root.to_string(),
        templates: program
            .templates
            .iter()
            .map(|text| (*text).to_string())
            .collect(),
        model: model.to_string(),
        values,
        tree,
        id: next_version(),
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
    let values = validated_values(&valid, &snapshot)?;
    let (tree, _) = tree_of(&valid, &values, false, Some(&previous.tree.root))?;
    let patches = crate::patch::diff(&previous.tree, &tree);
    Ok(Update {
        render: Render {
            root: previous.root.clone(),
            templates: previous.templates.clone(),
            model: previous.model.clone(),
            values,
            tree,
            id: next_version(),
        },
        patches,
    })
}

/// Updates `previous` by `changes` to its snapshot, not a whole new one
/// (docs/manual/runtime.md, "Changes"): the same result as an update to the
/// snapshot the changes make, at the cost of the changes and what depends on
/// them. The new snapshot is the previous render's values with the changes
/// applied, sharing what they don't touch, and only the values the changes give
/// are validated.
///
/// `changes.base` must be `previous`'s version, or nothing is done
/// (`runtime-changes-base-mismatch`). With `verify`, the snapshot the host
/// believes it now has, whole, is validated and compared with what the changes
/// make, and any difference is `runtime-changes-disagree`, at the first path
/// that differs: a check of whoever computed the changes, which costs a
/// validation of the whole snapshot, so it is for tests and development.
///
/// On diagnostics, `previous` is untouched and still valid.
pub fn update_changes(
    previous: &Render,
    changes: &crate::changes::Changes,
    verify: Option<&HostRecord>,
) -> Result<Update, Vec<RuntimeDiagnostic>> {
    if changes.base != previous.id {
        return Err(vec![RuntimeDiagnostic::new(
            RuntimeCode::CHANGES_BASE_MISMATCH,
            format!(
                "these changes are for render {}, but this is render {}: they were computed against another snapshot, and are not applied",
                changes.base, previous.id
            ),
            crate::diagnostic::Location::Input(vec![PathSegment::Name("base".to_string())]),
        )]);
    }
    let templates: Vec<&str> = previous.templates.iter().map(String::as_str).collect();
    let valid = program::validate(
        &Program {
            root: &previous.root,
            templates: &templates,
        },
        &previous.model,
    )?;
    let mut values = previous.values.clone();
    crate::changes::apply(&valid, &mut values, &changes.changes)?;
    if let Some(whole) = verify {
        let believed = validated_values(&valid, whole)?;
        if let Some(path) = crate::changes::first_difference(&values, &believed) {
            return Err(vec![RuntimeDiagnostic::new(
                RuntimeCode::CHANGES_DISAGREE,
                format!(
                    "the changes make a snapshot that differs from the whole snapshot given, at `{}`",
                    crate::boundary::display(&path)
                ),
                crate::diagnostic::Location::Input(path),
            )]);
        }
    }
    let (tree, _) = tree_of(&valid, &values, false, Some(&previous.tree.root))?;
    let patches = crate::patch::diff(&previous.tree, &tree);
    Ok(Update {
        render: Render {
            root: previous.root.clone(),
            templates: previous.templates.clone(),
            model: previous.model.clone(),
            values,
            tree,
            id: next_version(),
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
fn free_names<'t>(element: &'t Element, bound: &mut Vec<&'t str>, out: &mut BTreeSet<&'t str>) {
    let repeated = element.component == REPEAT_COMPONENT;
    // A repeat's `items` is read outside the name it binds; its `key` and its
    // child are read inside it.
    let mut own = BTreeSet::new();
    for prop in element
        .props
        .iter()
        .filter(|prop| !(repeated && prop.prop == "key"))
    {
        reads(&prop.value, &mut own);
    }
    out.extend(own.into_iter().filter(|name| !bound.contains(name)));
    if repeated {
        let name = repeat_name(element).expect("validated: a repeat has an `as`");
        bound.push(name);
        for prop in element.props.iter().filter(|prop| prop.prop == "key") {
            let mut read = BTreeSet::new();
            reads(&prop.value, &mut read);
            out.extend(read.into_iter().filter(|name| !bound.contains(name)));
        }
        for child in alternatives(element) {
            free_names(child, bound, out);
        }
        bound.pop();
    } else if element.component == CONDITIONAL_COMPONENT {
        for alternative in alternatives(element) {
            free_names(alternative, bound, out);
        }
    } else {
        // A primitive's children, or the children of a composite occurrence,
        // which its template places in the caller's scope: this one.
        for child in &element.children {
            match child {
                Child::Element { element } => free_names(element, bound, out),
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

/// Whether a handler binding names something that handles it. A command of
/// the template's component does. An event of it does not by itself: the
/// handler *forwards* the event to the occurrence of the component, so it
/// is live only if that occurrence binds the event, to a command of the
/// component whose template the occurrence is in, or to a forward in turn,
/// which is live by the same rule one occurrence out. `chain` is the
/// occurrences around this template, outermost first, each with the
/// component whose template it is in. A binding that isn't live isn't in the
/// render tree: the node has no such event, so event resolution (§9.9) goes on
/// to the nodes above it.
pub(crate) fn is_live(
    valid: &Valid,
    component: &str,
    command: &str,
    chain: &[(&str, &Element)],
) -> bool {
    if valid.component(component).commands.contains_key(command) {
        return true;
    }
    let Some(((caller, occurrence), outer)) = chain.split_last() else {
        return false;
    };
    occurrence
        .events
        .iter()
        .find(|binding| binding.event == command)
        .is_some_and(|binding| is_live(valid, caller, &binding.command, outer))
}

/// One composite occurrence around a recorded handler: the component whose
/// template it is in, the occurrence, and the scope it was written in.
pub(crate) struct Link<'v> {
    pub component: String,
    pub occurrence: &'v Element,
    pub values: BTreeMap<String, Value>,
}

/// A composite occurrence being rendered: whose children a `mesh-slot` in
/// its template places, in what scope.
struct Frame<'v> {
    occurrence: &'v Element,
    /// The component whose template the occurrence is in.
    component: String,
    /// The caller's scope at the occurrence; only kept when the occurrence has children.
    values: Option<BTreeMap<String, Value>>,
}

/// What rendering a list of children gives.
#[derive(Default)]
struct Built {
    children: Vec<TreeChild>,
    runs: Vec<Run>,
    repeats: Vec<RepeatMemo>,
}

/// Whether the subtree of `element` places a slot (its own, or one it forwards
/// to a composite's children): what it renders then depends on the caller's
/// content, which its own memo doesn't record, so it is rebuilt, never kept.
fn contains_slot(element: &Element) -> bool {
    element.component == SLOT_COMPONENT
        || element.children.iter().any(|child| match child {
            Child::Element { element } => contains_slot(element),
            _ => false,
        })
}

/// Runs of text a slot left side by side are one run (a render tree's text
/// runs are maximal, §9.8), named by the first.
fn coalesce(children: &mut Vec<TreeChild>) {
    // Nearly always there is nothing to merge: look before building a list.
    if !children
        .windows(2)
        .any(|pair| matches!(pair, [TreeChild::Text { .. }, TreeChild::Text { .. }]))
    {
        return;
    }

    let mut merged: Vec<TreeChild> = Vec::with_capacity(children.len());
    for child in children.drain(..) {
        match (merged.last_mut(), child) {
            (Some(TreeChild::Text { text, .. }), TreeChild::Text { text: more, .. }) => {
                text.push_str(&more);
            }
            (_, child) => merged.push(child),
        }
    }
    *children = merged;
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
    /// The composite occurrences around the node, outermost first, with the
    /// scopes they were written in, when the handler forwards an event: where
    /// it goes next. Empty otherwise.
    pub chain: Vec<Link<'v>>,
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
    let scope = validated_values(valid, snapshot)?;
    tree_of(valid, &scope, record, previous)
}

/// The root scope's values from a snapshot, validated (§9.8.4): every
/// mismatch is reported, at its path.
pub(crate) fn validated_values(
    valid: &Valid,
    snapshot: &HostRecord,
) -> Result<BTreeMap<String, Value>, Vec<RuntimeDiagnostic>> {
    let mut inputs = Inputs::new(&valid.manifest);
    let scope = snapshot_values(valid, snapshot, &mut inputs);
    let diagnostics = inputs.sorted();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(scope)
}

/// The render tree of a validated program against validated root values
/// (`previous`, when updating, is the root node of the tree it updates).
pub(crate) fn tree_of<'v>(
    valid: &'v Valid,
    scope: &BTreeMap<String, Value>,
    record: bool,
    previous: Option<&Rc<Node>>,
) -> Result<(Tree, Sites<'v>), Vec<RuntimeDiagnostic>> {
    let mut renderer = Renderer {
        valid,
        keys: HashSet::new(),
        sites: record.then(BTreeMap::new),
        free: HashMap::new(),
        prefixes: Vec::new(),
        frames: Vec::new(),
        site: Vec::new(),
        slots: HashMap::new(),
    };
    let root = &valid.templates[&valid.root];
    let old: Old<'_> = previous
        .map(|root| HashMap::from([(root.key.as_str(), root)]))
        .unwrap_or_default();
    let tree = Tree {
        root: renderer
            .occurrence(&valid.root, &root.root, 0, &mut Vec::new(), scope, &old)
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
    /// Where the children of the enclosing composite's occurrence go: the
    /// caller's content, rendered in the caller's scope. The name is the
    /// slot's (empty for the default slot).
    Slot(&'t str),
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
                // A fill is content for a named slot, never placed where it is written.
                if element.component == FILL_COMPONENT {
                    continue;
                }
                children.push(if element.component == CONDITIONAL_COMPONENT {
                    RenderChild::Conditional(element)
                } else if element.component == REPEAT_COMPONENT {
                    RenderChild::Repeat(element)
                } else if element.component == SLOT_COMPONENT {
                    RenderChild::Slot(program::slot_name(element))
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
    /// The composite occurrences being rendered, innermost last.
    frames: Vec<Frame<'v>>,
    /// Where in the node being rendered (positions, through any slots) the
    /// children being rendered are: names a repeat among them.
    site: Vec<usize>,
    /// Whether each element's subtree places a slot.
    slots: HashMap<*const Element, bool>,
}

impl<'v> Renderer<'v> {
    /// The scope names the subtree of `element` reads from outside it.
    fn free(&mut self, element: &'v Element) -> Rc<Vec<String>> {
        Rc::clone(self.free.entry(element).or_insert_with(|| {
            let mut names = BTreeSet::new();
            free_names(element, &mut Vec::new(), &mut names);
            Rc::new(names.into_iter().map(str::to_string).collect())
        }))
    }

    fn has_slot(&mut self, element: &'v Element) -> bool {
        *self
            .slots
            .entry(element)
            .or_insert_with(|| contains_slot(element))
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
            // The occurrence's children are the caller's content: a slot in the
            // template places them, in the scope they were written in (this one).
            self.frames.push(Frame {
                occurrence: element,
                component: component.to_string(),
                values: (!element.children.is_empty() || !element.events.is_empty())
                    .then(|| values.clone()),
            });
            let rendered =
                self.occurrence(&template.component, &template.root, 0, path, &bound, old);
            self.frames.pop();
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
        // Unless it places a slot: what it renders then depends on the
        // caller's content too, which its memo doesn't record.
        let hosts_slot = self.has_slot(element);
        if let Some(previous) = previous.filter(|_| !hosts_slot) {
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
        // A binding that forwards an event nothing handles is not in the tree.
        let own_commands = &self.valid.component(scope.component).commands;
        let chain: Vec<(&str, &Element)> = if element
            .events
            .iter()
            .any(|binding| !own_commands.contains_key(&binding.command))
        {
            self.frames
                .iter()
                .map(|frame| (frame.component.as_str(), frame.occurrence))
                .collect()
        } else {
            Vec::new()
        };
        let live: Vec<&mpl::EventBinding> = element
            .events
            .iter()
            .filter(|binding| is_live(self.valid, scope.component, &binding.command, &chain))
            .collect();
        let events: BTreeMap<String, String> = live
            .iter()
            .map(|binding| {
                (
                    binding.event.clone(),
                    program::handler(&self.valid.identity, &key, &binding.event),
                )
            })
            .collect();
        if let Some(sites) = &mut self.sites {
            for binding in &live {
                let forwards = !own_commands.contains_key(&binding.command);
                sites.insert(
                    events[&binding.event].clone(),
                    Recorded {
                        component: scope.component.to_string(),
                        node: element,
                        event: &binding.event,
                        values: scope.values.clone(),
                        chain: if forwards {
                            self.frames
                                .iter()
                                .map(|frame| Link {
                                    component: frame.component.clone(),
                                    occurrence: frame.occurrence,
                                    values: frame.values.clone().unwrap_or_default(),
                                })
                                .collect()
                        } else {
                            Vec::new()
                        },
                    },
                );
            }
        }
        let old_below = old_children(previous);
        let mut built = Built::default();
        let outer_site = std::mem::take(&mut self.site);
        let listed = self.render_list(scope, element, path, &old_below, previous, &mut built);
        self.site = outer_site;
        listed?;
        // Text left side by side (by a slot, a fragment, or a conditional or repeat that chose nothing) is one run: a render tree's runs are maximal.
        coalesce(&mut built.children);
        let Built {
            children,
            runs,
            repeats,
            ..
        } = built;
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

    /// Renders the children of `source` (an element of the template whose
    /// scope is `scope`'s) into `built`: for a node, its own; for a slot, the
    /// children of the composite occurrence that placed it.
    fn render_list(
        &mut self,
        scope: &Scope<'_, '_>,
        source: &'v Element,
        path: &mut Vec<Step>,
        old_below: &Old<'_>,
        previous: Option<&Rc<Node>>,
        built: &mut Built,
    ) -> Result<(), RuntimeDiagnostic> {
        for (position, child) in render_children(source).into_iter().enumerate() {
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
                    let key = self.key(path, scope, source.span);
                    path.pop();
                    let key = key?;
                    built
                        .runs
                        .push((key.clone(), inputs_of(&read, scope.values), text.clone()));
                    built.children.push(TreeChild::Text { key, text });
                }
                RenderChild::Element(child) => {
                    self.place(scope, child, position, path, old_below, previous, built)?;
                }
                RenderChild::Conditional(conditional) => {
                    // The slot is `position` whether or not a node comes of it.
                    if let Some((alternative, chosen)) = self.choose(scope, conditional)? {
                        path.push(Step {
                            position,
                            kind: CONDITIONAL,
                            component: ALTERNATIVES[alternative].to_string(),
                        });
                        let placed = self.place(scope, chosen, 0, path, old_below, previous, built);
                        path.pop();
                        placed?;
                    }
                }
                RenderChild::Repeat(repeat) => {
                    let memo =
                        self.repeat(scope, repeat, position, path, built, old_below, previous)?;
                    built.repeats.push(memo);
                }
                RenderChild::Slot(name) => {
                    self.slot(position, name, path, old_below, previous, built)?;
                }
            }
        }
        Ok(())
    }

    /// Places `element`, an occurrence at `position` in the template of `scope`'s component: a node, as one child of `built`; or, for what makes no node of its own (a
    /// `mesh-fragment`, or a composite whose template starts with one), its content, in place, as the slot places the caller's.
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        scope: &Scope<'_, '_>,
        element: &'v Element,
        position: usize,
        path: &mut Vec<Step>,
        old: &Old<'_>,
        previous: Option<&Rc<Node>>,
        built: &mut Built,
    ) -> Result<(), RuntimeDiagnostic> {
        if !program::is_inline(self.valid, element) {
            built.children.push(TreeChild::Node(self.occurrence(
                scope.component,
                element,
                position,
                path,
                scope.values,
                old,
            )?));

            return Ok(());
        }

        self.site.push(position);

        let placed = if element.component == FRAGMENT_COMPONENT {
            path.push(Step {
                position,
                kind: FRAGMENT,
                component: String::new(),
            });
            let listed = self.render_list(scope, element, path, old, previous, built);
            path.pop();
            listed
        } else {
            let bound = bind(self.valid, scope, element);
            match bound {
                Err(diagnostic) => Err(diagnostic),
                Ok(bound) => {
                    let template = &self.valid.templates[&element.component];
                    path.push(Step {
                        position,
                        kind: COMPOSITE,
                        component: element.component.clone(),
                    });
                    self.frames.push(Frame {
                        occurrence: element,
                        component: scope.component.to_string(),
                        values: (!element.children.is_empty() || !element.events.is_empty())
                            .then(|| scope.values.clone()),
                    });
                    let inner = Scope {
                        component: &template.component,
                        values: &bound,
                        payload: None,
                        statics: statics(self.valid, &template.component, None),
                    };
                    let placed = self.place(&inner, &template.root, 0, path, old, previous, built);
                    self.frames.pop();
                    path.pop();
                    placed
                }
            }
        };

        self.site.pop();
        placed
    }

    /// A slot: the children of the innermost composite occurrence, rendered in
    /// the scope of its caller, under a step of their own (the slot's), and
    /// with the occurrences outside it, not it, as the ones a slot among them places.
    fn slot(
        &mut self,
        position: usize,
        name: &str,
        path: &mut Vec<Step>,
        old_below: &Old<'_>,
        previous: Option<&Rc<Node>>,
        built: &mut Built,
    ) -> Result<(), RuntimeDiagnostic> {
        let Some(frame) = self.frames.pop() else {
            return Ok(()); // a template with a slot that is the root: no caller
        };
        // The default slot places the occurrence's own children (fills aside); a named one, its fill's.
        let content = if name.is_empty() {
            Some(frame.occurrence)
        } else {
            program::fill_for(frame.occurrence, name)
        };
        let result = match (&frame.values, content) {
            (None, _) | (_, None) => Ok(()), // an occurrence with no children, or none for this slot
            (Some(values), Some(content)) => {
                path.push(Step {
                    position,
                    kind: SLOT,
                    component: name.to_string(),
                });
                self.site.push(position);
                let scope = Scope {
                    component: &frame.component,
                    values,
                    payload: None,
                    statics: statics(self.valid, &frame.component, None),
                };
                let listed = self.render_list(&scope, content, path, old_below, previous, built);
                self.site.pop();
                path.pop();
                listed
            }
        };
        self.frames.push(frame);
        result
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
        built: &mut Built,
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
        let mut site = self.site.clone();
        site.push(position);
        let hosts_slot = self.has_slot(child);
        let before = previous
            .and_then(|previous| previous.memo.repeats.iter().find(|memo| memo.site == site))
            .filter(|memo| unchanged(&memo.key_other, scope.values))
            .filter(|_| !hosts_slot);
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
                .filter(|kept| kept.item.identical(item) && unchanged(&kept.free, &values))
            {
                if !seen.insert(Rc::clone(&kept.canonical)) {
                    return Err(scope.error(
                        RuntimeCode::DUPLICATE_KEY,
                        "two items of this repeat declare the same key",
                        key.value.span(),
                    ));
                }
                built.children.extend(kept.children.iter().cloned());
                built.runs.extend(kept.runs.iter().cloned());
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
            let made_item = if program::is_inline(self.valid, child) {
                // An item that is a fragment places its content: what it made is its children, which it keeps as the node would be kept. A repeat inside it is
                // not remembered (the items share a template position, so they would share a site): it is made again.
                let mut inner_built = Built::default();
                let placed = self.place(&inner, child, 0, path, old, previous, &mut inner_built);
                path.pop();
                placed?;
                let names = self.free(child);
                let free = inputs_of(&names.iter().map(String::as_str).collect(), &values);
                RepeatItem {
                    canonical,
                    item: item.clone(),
                    children: inner_built.children,
                    free,
                    runs: inner_built.runs,
                }
            } else {
                let rendered = self.occurrence(scope.component, child, 0, path, &values, old);
                path.pop();
                let rendered = rendered?;
                RepeatItem {
                    canonical,
                    item: item.clone(),
                    free: rendered.memo.free.clone(),
                    children: vec![TreeChild::Node(rendered)],
                    runs: Vec::new(),
                }
            };
            built.children.extend(made_item.children.iter().cloned());
            built.runs.extend(made_item.runs.iter().cloned());
            made.push(made_item);
        }
        self.prefixes.pop();
        Ok(RepeatMemo {
            site,
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
