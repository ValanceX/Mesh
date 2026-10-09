//! Changes (docs/manual/runtime.md, "Changes"): an update from a render and
//! the changes to its snapshot, not a whole new snapshot.
//!
//! The changes are an ordered list of edits at paths into the snapshot
//! (`set`, `insert`, `remove`), and they **define** the new snapshot: the
//! runtime applies them to the values of the render they name, so the new
//! snapshot is never a second copy that could disagree with them. What they
//! don't touch is shared with the previous render (the values are `Rc`s), so
//! an update costs the edits and what depends on them, not the snapshot, and
//! the renderer's comparison of an unchanged value is a pointer comparison.
//! Only a value an edit gives is validated: the rest was validated when the
//! previous render was made, and cannot have changed.
//!
//! Every change is checked before anything is done: the render it names is
//! the one given (`base`), every path leads somewhere the type allows, and
//! every value fits its type. A change that fails any of that is a
//! diagnostic, and no render is made; the previous one is untouched.

use crate::boundary::{display, Inputs};
use crate::diagnostic::{Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::program::Valid;
use crate::types::reads_as_optional;
use crate::value::{HostValue, Value};
use mesh_manifest::{Manifest, Type};
use std::collections::BTreeMap;
use std::rc::Rc;

/// One edit of a snapshot.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// The value at `path` is now `value`: a scope name's, a record field's
    /// (made if the record doesn't have it, the type allowing), or a list
    /// element's (the list's length appends one).
    Set {
        path: Vec<PathSegment>,
        value: HostValue,
    },
    /// `value` is now the list element at `path`'s index, and the elements
    /// from it on are one later.
    Insert {
        path: Vec<PathSegment>,
        value: HostValue,
    },
    /// The record field or list element at `path` is gone (a list's later
    /// elements are one earlier). A field must be one the type lets be absent.
    Remove { path: Vec<PathSegment> },
}

/// The edits to a render's snapshot, and the render they are for.
#[derive(Debug, Clone, PartialEq)]
pub struct Changes {
    /// The version ([`crate::Render::version`]) of the render these were
    /// computed against. Changes for any other render are refused.
    pub base: u64,
    /// The edits, applied in order: an edit sees the effect of those before it.
    pub changes: Vec<Change>,
}

fn invalid(path: &[PathSegment], message: impl Into<String>) -> RuntimeDiagnostic {
    RuntimeDiagnostic::new(
        RuntimeCode::INVALID_CHANGE,
        message,
        Location::Input(path.to_vec()),
    )
}

fn name(text: &str) -> PathSegment {
    PathSegment::Name(text.to_string())
}

impl Changes {
    /// Reads the changes document the host gave, as a boundary value (§9.8):
    /// `{ base, changes: [{ op, path, value? }] }`, where `op` is `"set"`,
    /// `"insert"` or `"remove"`, `path` is a non-empty list of record field
    /// names (strings) and list indices (non-negative integers), and `value`
    /// is there for `set` and `insert`, and not for `remove`.
    pub fn from_host(document: &HostValue) -> Result<Changes, Vec<RuntimeDiagnostic>> {
        let HostValue::Record(record) = document else {
            return Err(vec![invalid(
                &[],
                "the changes are a record: { base, changes }",
            )]);
        };
        let base = match record.get("base") {
            Some(HostValue::Number(base)) if integer(*base) => *base as u64,
            _ => {
                return Err(vec![invalid(
                &[name("base")],
                "`base` is the version of the render the changes are for: a non-negative integer",
            )])
            }
        };
        let Some(HostValue::List(list)) = record.get("changes") else {
            return Err(vec![invalid(
                &[name("changes")],
                "`changes` is a list of edits",
            )]);
        };
        let mut changes = Vec::with_capacity(list.len());
        for (index, edit) in list.iter().enumerate() {
            let at = [name("changes"), PathSegment::Index(index)];
            let Some(HostValue::Record(edit)) = edit else {
                return Err(vec![invalid(
                    &at,
                    "an edit is a record: { op, path, value? }",
                )]);
            };
            let path = match edit.get("path") {
                Some(HostValue::List(segments)) if !segments.is_empty() => {
                    let mut path = Vec::with_capacity(segments.len());
                    for (position, segment) in segments.iter().enumerate() {
                        path.push(match segment {
                            Some(HostValue::String(text)) => PathSegment::Name(text.clone()),
                            Some(HostValue::Number(number)) if integer(*number) => {
                                PathSegment::Index(*number as usize)
                            }
                            _ => {
                                let mut where_ = at.to_vec();
                                where_.extend([name("path"), PathSegment::Index(position)]);
                                return Err(vec![invalid(
                                    &where_,
                                    "a path segment is a field name (a string) or a list index (a non-negative integer)",
                                )]);
                            }
                        });
                    }
                    path
                }
                _ => {
                    let mut where_ = at.to_vec();
                    where_.push(name("path"));
                    return Err(vec![invalid(
                        &where_,
                        "`path` is a non-empty list of segments",
                    )]);
                }
            };
            let value = edit.get("value");
            let wants_value = |op: &str| {
                value.cloned().ok_or_else(|| {
                    let mut where_ = at.to_vec();
                    where_.push(name("value"));
                    vec![invalid(&where_, format!("`{op}` needs a `value`"))]
                })
            };
            changes.push(match edit.get("op") {
                Some(HostValue::String(op)) if op == "set" => Change::Set {
                    path,
                    value: wants_value("set")?,
                },
                Some(HostValue::String(op)) if op == "insert" => Change::Insert {
                    path,
                    value: wants_value("insert")?,
                },
                Some(HostValue::String(op)) if op == "remove" => {
                    if value.is_some() {
                        let mut where_ = at.to_vec();
                        where_.push(name("value"));
                        return Err(vec![invalid(&where_, "`remove` takes no `value`")]);
                    }
                    Change::Remove { path }
                }
                _ => {
                    let mut where_ = at.to_vec();
                    where_.push(name("op"));
                    return Err(vec![invalid(
                        &where_,
                        "`op` is \"set\", \"insert\" or \"remove\"",
                    )]);
                }
            });
        }
        Ok(Changes { base, changes })
    }
}

/// A non-negative integer a number can hold exactly.
fn integer(number: f64) -> bool {
    number >= 0.0 && number.fract() == 0.0 && number < 9_007_199_254_740_992.0
}

/// What an edit does at the end of its path.
#[derive(Clone, Copy)]
enum Op<'a> {
    Set(&'a HostValue),
    Insert(&'a HostValue),
    Remove,
}

struct Editor<'m> {
    manifest: &'m Manifest,
    inputs: Inputs<'m>,
}

/// The type of a value nothing is declared about.
const ANY: Type = Type::Any;

impl Editor<'_> {
    /// The new value of `current`, of type `ty`, after the edit `op` at `rest`
    /// below it. `path` is the path to `current`, for messages.
    fn edit(
        &mut self,
        current: &Value,
        ty: &Type,
        rest: &[PathSegment],
        op: Op<'_>,
        path: &mut Vec<PathSegment>,
    ) -> Result<Value, RuntimeDiagnostic> {
        let mut ty = self.manifest.expand(ty);
        if let Type::Optional(inner) = ty {
            ty = self.manifest.expand(inner);
        }
        let (first, tail) = rest
            .split_first()
            .expect("an edit below a value has a path");
        path.push(first.clone());
        let result = self.below(current, ty, first, tail, op, path);
        path.pop();
        result
    }

    fn below(
        &mut self,
        current: &Value,
        ty: &Type,
        first: &PathSegment,
        tail: &[PathSegment],
        op: Op<'_>,
        path: &mut Vec<PathSegment>,
    ) -> Result<Value, RuntimeDiagnostic> {
        let here = display(path);
        match (current, first) {
            (Value::Record(fields), PathSegment::Name(field)) => {
                let declared = match ty {
                    Type::Record(declared) => match declared.get(field) {
                        Some(declared) => Some(declared),
                        None => {
                            return Err(invalid(
                                path,
                                format!(
                                    "`{here}` isn't a field of its record type: records are exact"
                                ),
                            ))
                        }
                    },
                    Type::Any => None,
                    _ => return Err(invalid(path, format!("`{here}` is not inside a record"))),
                };
                let field_type = declared.map_or(&ANY, |declared| &declared.ty);
                let mut next = (**fields).clone();
                if tail.is_empty() {
                    match op {
                        Op::Set(host) => {
                            let value = self.inputs.value(Some(host), field_type, path);
                            next.insert(field.clone(), value);
                        }
                        Op::Remove => {
                            if !fields.contains_key(field) {
                                return Err(invalid(path, format!("`{here}` is already absent")));
                            }
                            if declared
                                .is_some_and(|declared| !reads_as_optional(self.manifest, declared))
                            {
                                return Err(invalid(
                                    path,
                                    format!("`{here}` is a required field, and can't be removed"),
                                ));
                            }
                            next.remove(field);
                        }
                        Op::Insert(_) => {
                            return Err(invalid(
                                path,
                                format!("`{here}` is a record field: it is set, not inserted"),
                            ))
                        }
                    }
                } else {
                    let child = fields.get(field).ok_or_else(|| {
                        invalid(
                            path,
                            format!("`{here}` is absent, so there is nothing below it"),
                        )
                    })?;
                    let edited = self.edit(child, field_type, tail, op, path)?;
                    next.insert(field.clone(), edited);
                }
                Ok(Value::Record(Rc::new(next)))
            }
            (Value::List(items), PathSegment::Index(index)) => {
                let element = match ty {
                    Type::List(element) => &**element,
                    Type::Any => &ANY,
                    _ => return Err(invalid(path, format!("`{here}` is not inside a list"))),
                };
                let (index, len) = (*index, items.len());
                let mut next: Vec<Value> = items.to_vec();
                if tail.is_empty() {
                    match op {
                        Op::Set(host) if index < len => {
                            next[index] = self.inputs.value(Some(host), element, path);
                        }
                        Op::Set(host) if index == len => {
                            next.push(self.inputs.value(Some(host), element, path));
                        }
                        Op::Insert(host) if index <= len => {
                            next.insert(index, self.inputs.value(Some(host), element, path));
                        }
                        Op::Remove if index < len => {
                            next.remove(index);
                        }
                        _ => {
                            return Err(invalid(
                                path,
                                format!("`{here}` is past the end of a list of {len}"),
                            ))
                        }
                    }
                } else {
                    let child = items.get(index).ok_or_else(|| {
                        invalid(path, format!("`{here}` is past the end of a list of {len}"))
                    })?;
                    next[index] = self.edit(child, element, tail, op, path)?;
                }
                Ok(Value::List(next.into()))
            }
            (Value::Record(_), PathSegment::Index(_)) => Err(invalid(
                path,
                format!("`{here}` is a list index, but its parent is a record"),
            )),
            (Value::List(_), PathSegment::Name(_)) => Err(invalid(
                path,
                format!("`{here}` is a field name, but its parent is a list"),
            )),
            _ => Err(invalid(
                path,
                format!("`{here}` is below a value that has no fields or elements"),
            )),
        }
    }
}

/// Applies `changes`, in order, to `values` (the root scope's values),
/// checking each before it is applied. On the first failure `values` is
/// unchanged and the failure is returned.
pub(crate) fn apply(
    valid: &Valid,
    values: &mut BTreeMap<String, Value>,
    changes: &[Change],
) -> Result<(), Vec<RuntimeDiagnostic>> {
    let scope = &valid.component(&valid.root).scope;
    let mut working = values.clone();
    for (index, change) in changes.iter().enumerate() {
        let (path, op) = match change {
            Change::Set { path, value } => (path, Op::Set(value)),
            Change::Insert { path, value } => (path, Op::Insert(value)),
            Change::Remove { path } => (path, Op::Remove),
        };
        let at = |message: String| {
            vec![RuntimeDiagnostic::new(
                RuntimeCode::INVALID_CHANGE,
                format!("change {index}: {message}"),
                Location::Input(path.clone()),
            )]
        };
        let Some(PathSegment::Name(root)) = path.first() else {
            return Err(at("a path starts with a scope name".into()));
        };
        let Some(ty) = scope.get(root) else {
            return Err(at(format!("`{root}` isn't a scope name of the root")));
        };
        let mut editor = Editor {
            manifest: &valid.manifest,
            inputs: Inputs::new(&valid.manifest),
        };
        let mut where_ = vec![name(root)];
        let edited = if path.len() == 1 {
            match op {
                Op::Set(host) => editor.inputs.value(Some(host), ty, &mut where_),
                Op::Remove => {
                    if !matches!(valid.manifest.expand(ty), Type::Optional(_)) {
                        return Err(at(format!("`{root}` is required, and can't be removed")));
                    }
                    Value::Absent
                }
                Op::Insert(_) => {
                    return Err(at(format!(
                        "`{root}` is a scope name: it is set, not inserted"
                    )))
                }
            }
        } else {
            let current = working.get(root).cloned().unwrap_or(Value::Absent);
            match editor.edit(&current, ty, &path[1..], op, &mut where_) {
                Ok(value) => value,
                Err(diagnostic) => {
                    return Err(vec![RuntimeDiagnostic::new(
                        RuntimeCode::INVALID_CHANGE,
                        format!("change {index}: {}", diagnostic.message),
                        diagnostic.location,
                    )])
                }
            }
        };
        let diagnostics = editor.inputs.sorted();
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        working.insert(root.clone(), edited);
    }
    *values = working;
    Ok(())
}

/// The path of the first difference between two root scopes, if they differ:
/// by bits, so `0` and `-0` differ.
pub(crate) fn first_difference(
    a: &BTreeMap<String, Value>,
    b: &BTreeMap<String, Value>,
) -> Option<Vec<PathSegment>> {
    let names: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for scope_name in names {
        let (x, y) = (
            a.get(scope_name).unwrap_or(&Value::Absent),
            b.get(scope_name).unwrap_or(&Value::Absent),
        );
        let mut path = vec![PathSegment::Name(scope_name.clone())];
        if differ(x, y, &mut path) {
            return Some(path);
        }
    }
    None
}

/// Whether `x` and `y` differ, leaving `path` at the first difference.
fn differ(x: &Value, y: &Value, path: &mut Vec<PathSegment>) -> bool {
    if x.identical(y) {
        return false;
    }
    match (x, y) {
        (Value::Record(x), Value::Record(y)) => {
            let names: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for field in names {
                path.push(PathSegment::Name(field.clone()));
                if differ(
                    x.get(field).unwrap_or(&Value::Absent),
                    y.get(field).unwrap_or(&Value::Absent),
                    path,
                ) {
                    return true;
                }
                path.pop();
            }
            true
        }
        (Value::List(x), Value::List(y)) => {
            for index in 0..x.len().max(y.len()) {
                path.push(PathSegment::Index(index));
                if differ(
                    x.get(index).unwrap_or(&Value::Absent),
                    y.get(index).unwrap_or(&Value::Absent),
                    path,
                ) {
                    return true;
                }
                path.pop();
            }
            true
        }
        _ => true,
    }
}
