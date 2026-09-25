//! The boundary (§9.8): host values into evaluator values, checked
//! against their types (input validation), and evaluator values out, to
//! the boundary data model (the output check).

use crate::diagnostic::{Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::types::reads_as_optional;
use crate::value::{HostKey, HostValue, Value};
use mesh_manifest::{Manifest, Type};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Input validation's state: every mismatch found, each at its path.
pub(crate) struct Inputs<'m> {
    manifest: &'m Manifest,
    pub(crate) diagnostics: Vec<RuntimeDiagnostic>,
}

/// `user.name`, `users[1]`, `$event.x`: a path as a message shows it.
pub(crate) fn display(path: &[PathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            PathSegment::Name(name) if text.is_empty() => text.push_str(name),
            PathSegment::Name(name) => {
                text.push('.');
                text.push_str(name);
            }
            PathSegment::Index(index) => text.push_str(&format!("[{index}]")),
        }
    }
    text
}

fn host_kind(value: &HostValue) -> &'static str {
    match value {
        HostValue::Null => "null",
        HostValue::Boolean(_) => "a boolean",
        HostValue::Number(_) | HostValue::OutOfRange(_) => "a number",
        HostValue::String(_) | HostValue::Utf16(_) => "a string",
        HostValue::List(_) => "a list",
        HostValue::Record(_) => "a record",
        HostValue::Unsupported(_) => "an unsupported value",
    }
}

fn expected(ty: &Type) -> &'static str {
    match ty {
        Type::String => "a string",
        Type::Number => "a number",
        Type::Boolean => "a boolean",
        Type::Null => "null",
        Type::List(_) => "a list",
        Type::Record(_) => "a record",
        Type::Any | Type::Named(_) | Type::Optional(_) => "a value",
    }
}

impl<'m> Inputs<'m> {
    pub(crate) fn new(manifest: &'m Manifest) -> Self {
        Inputs {
            manifest,
            diagnostics: Vec::new(),
        }
    }

    fn report(&mut self, code: RuntimeCode, path: &[PathSegment], message: String) {
        self.diagnostics.push(RuntimeDiagnostic::new(
            code,
            message,
            Location::Input(path.to_vec()),
        ));
    }

    /// `host` (absent when `None`) as a value of type `ty`, at `path`.
    /// Every mismatch is reported; the value returned is meaningful only
    /// when none was.
    pub(crate) fn value(
        &mut self,
        host: Option<&HostValue>,
        ty: &Type,
        path: &mut Vec<PathSegment>,
    ) -> Value {
        let ty = self.manifest.expand(ty);
        if let Type::Optional(inner) = ty {
            return match host {
                None => Value::Absent,
                Some(host) => self.value(Some(host), inner, path),
            };
        }
        let Some(host) = host else {
            let what = display(path);
            self.report(
                RuntimeCode::MISSING_VALUE,
                path,
                format!("`{what}` is absent, but must be {}", expected(ty)),
            );
            return Value::Absent;
        };
        if !self.admissible(host, path) {
            return Value::Absent;
        }
        match (ty, host) {
            (Type::Any, _) => self.any(host, path),
            (Type::String, HostValue::String(text)) => Value::String(Rc::from(text.as_str())),
            (Type::Number, HostValue::Number(number)) => Value::Number(*number),
            (Type::Boolean, HostValue::Boolean(value)) => Value::Boolean(*value),
            (Type::Null, HostValue::Null) => Value::Null,
            (Type::List(element), HostValue::List(items)) => {
                let mut values = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    path.push(PathSegment::Index(index));
                    match item {
                        None => {
                            let what = display(path);
                            self.report(
                                RuntimeCode::ABSENT_ELEMENT,
                                path,
                                format!("`{what}` is absent: a list's elements are all present"),
                            );
                            values.push(Value::Absent);
                        }
                        Some(item) => values.push(self.value(Some(item), element, path)),
                    }
                    path.pop();
                }
                Value::List(values.into())
            }
            (Type::Record(fields), HostValue::Record(record)) => {
                let mut values = BTreeMap::new();
                let mut seen = Vec::new();
                for (key, value) in &record.0 {
                    let HostKey::Text(name) = key else {
                        self.unpaired_key(key, path);
                        continue;
                    };
                    path.push(PathSegment::Name(name.clone()));
                    if seen.contains(&name) {
                        let what = display(path);
                        self.report(
                            RuntimeCode::VALUE_MISMATCH,
                            path,
                            format!("`{what}` is given twice"),
                        );
                    } else if let Some(field) = fields.get(name) {
                        let value = self.value(Some(value), &field.ty, path);
                        if !matches!(value, Value::Absent) {
                            values.insert(name.clone(), value);
                        }
                    } else {
                        let what = display(path);
                        self.report(
                            RuntimeCode::UNKNOWN_FIELD,
                            path,
                            format!("`{what}` isn't a field of its record type: records are exact"),
                        );
                    }
                    seen.push(name);
                    path.pop();
                }
                for (name, field) in fields {
                    if record.get(name).is_none() && !reads_as_optional(self.manifest, field) {
                        path.push(PathSegment::Name(name.clone()));
                        let what = display(path);
                        self.report(
                            RuntimeCode::MISSING_VALUE,
                            path,
                            format!("`{what}` is absent, but its field is required"),
                        );
                        path.pop();
                    }
                }
                Value::Record(Rc::new(values))
            }
            _ => {
                let what = display(path);
                self.report(
                    RuntimeCode::VALUE_MISMATCH,
                    path,
                    format!(
                        "`{what}` is {}, but must be {}",
                        host_kind(host),
                        expected(ty)
                    ),
                );
                Value::Absent
            }
        }
    }

    /// Whether `host` itself is in the boundary data model, whatever its
    /// type; if not, reports why.
    fn admissible(&mut self, host: &HostValue, path: &[PathSegment]) -> bool {
        let what = display(path);
        let (code, message) = match host {
            HostValue::Utf16(_) => (
                RuntimeCode::UNPAIRED_SURROGATE,
                format!("`{what}` has an unpaired surrogate: strings are Unicode"),
            ),
            HostValue::OutOfRange(text) => (
                RuntimeCode::NUMBER_OUT_OF_RANGE,
                format!("`{what}` is {text}, too large for a number"),
            ),
            HostValue::Number(number) if !number.is_finite() => (
                RuntimeCode::NON_FINITE_INPUT,
                format!("`{what}` isn't a finite number"),
            ),
            HostValue::Unsupported(kind) => (
                RuntimeCode::UNSUPPORTED_VALUE,
                format!(
                    "`{what}` is {}, which MESH values can't be",
                    unsupported(kind)
                ),
            ),
            _ => return true,
        };
        self.report(code, path, message);
        false
    }

    fn unpaired_key(&mut self, key: &HostKey, path: &mut Vec<PathSegment>) {
        let HostKey::Utf16(units) = key else { return };
        path.push(PathSegment::Name(String::from_utf16_lossy(units)));
        let what = display(path);
        self.report(
            RuntimeCode::UNPAIRED_SURROGATE,
            path,
            format!("the field name `{what}` has an unpaired surrogate"),
        );
        path.pop();
    }

    /// A value of type `any`: any present value, still within the
    /// boundary data model at every depth (§9.8.3).
    fn any(&mut self, host: &HostValue, path: &mut Vec<PathSegment>) -> Value {
        if !self.admissible(host, path) {
            return Value::Absent;
        }
        match host {
            HostValue::Null => Value::Null,
            HostValue::Boolean(value) => Value::Boolean(*value),
            HostValue::Number(number) => Value::Number(*number),
            HostValue::String(text) => Value::String(Rc::from(text.as_str())),
            HostValue::List(items) => {
                let mut values = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    path.push(PathSegment::Index(index));
                    match item {
                        None => {
                            let what = display(path);
                            self.report(
                                RuntimeCode::ABSENT_ELEMENT,
                                path,
                                format!("`{what}` is absent: a list's elements are all present"),
                            );
                        }
                        Some(item) => values.push(self.any(item, path)),
                    }
                    path.pop();
                }
                Value::List(values.into())
            }
            HostValue::Record(record) => {
                let mut values = BTreeMap::new();
                for (key, value) in &record.0 {
                    let HostKey::Text(name) = key else {
                        self.unpaired_key(key, path);
                        continue;
                    };
                    path.push(PathSegment::Name(name.clone()));
                    if values.contains_key(name) {
                        let what = display(path);
                        self.report(
                            RuntimeCode::VALUE_MISMATCH,
                            path,
                            format!("`{what}` is given twice"),
                        );
                    } else {
                        values.insert(name.clone(), self.any(value, path));
                    }
                    path.pop();
                }
                Value::Record(Rc::new(values))
            }
            HostValue::Utf16(_) | HostValue::OutOfRange(_) | HostValue::Unsupported(_) => {
                unreachable!("refused by admissible")
            }
        }
    }

    /// The diagnostics, in D3's order: by path, the snapshot's before the
    /// payload's.
    pub(crate) fn sorted(mut self) -> Vec<RuntimeDiagnostic> {
        let key = |diagnostic: &RuntimeDiagnostic| match &diagnostic.location {
            Location::Input(path) => (
                matches!(path.first(), Some(PathSegment::Name(name)) if name == "$event"),
                path.clone(),
            ),
            _ => (false, Vec::new()),
        };
        self.diagnostics.sort_by_key(key);
        self.diagnostics
    }
}

/// Why a value can't be an output (§9.7.6's output check).
pub(crate) struct NotOutput {
    pub code: RuntimeCode,
    pub message: String,
}

/// `value` as an output, in the boundary data model: `-0` becomes `0`;
/// a non-finite number, or an absent list element, at any depth, is
/// refused. An absent value itself is the caller's to handle, since what
/// it means depends on where it goes (an omitted prop, an absent
/// argument, empty text).
pub(crate) fn output(value: &Value) -> Result<serde_json::Value, NotOutput> {
    Ok(match value {
        Value::Absent => unreachable!("the caller handles an absent output"),
        Value::Null => serde_json::Value::Null,
        Value::Boolean(value) => serde_json::Value::Bool(*value),
        Value::Number(number) => number_json(*number)?,
        Value::String(text) => serde_json::Value::String(text.to_string()),
        Value::List(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| match item {
                    Value::Absent => Err(NotOutput {
                        code: RuntimeCode::ABSENT_ELEMENT_OUTPUT,
                        message: "a list with an absent element can't leave the runtime".into(),
                    }),
                    item => output(item),
                })
                .collect::<Result<_, _>>()?,
        ),
        Value::Record(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(name, value)| Ok((name.clone(), output(value)?)))
                .collect::<Result<_, NotOutput>>()?,
        ),
    })
}

/// A finite number as JSON; `-0` is `0`, and an integer exact in
/// binary64 is written as one.
fn number_json(number: f64) -> Result<serde_json::Value, NotOutput> {
    if !number.is_finite() {
        return Err(NotOutput {
            code: RuntimeCode::NON_FINITE_OUTPUT,
            message: "a number that isn't finite (NaN or an infinity) can't leave the runtime"
                .into(),
        });
    }
    let number = if number == 0.0 { 0.0 } else { number };
    if number.fract() == 0.0 && number.abs() < 9_007_199_254_740_992.0 {
        #[allow(clippy::cast_possible_truncation)]
        return Ok(serde_json::Value::from(number as i64));
    }
    Ok(serde_json::Number::from_f64(number)
        .map(serde_json::Value::Number)
        .expect("finite"))
}

/// How a message names an unsupported value's kind, as a JavaScript host
/// reports it (§9.8.6): `function`, `symbol`, `bigint`, `cycle`, or
/// `object`, with `:` and the constructor's name when it has one.
fn unsupported(kind: &str) -> String {
    match kind {
        "function" | "symbol" => format!("a {kind}"),
        "bigint" => "a `bigint`".to_string(),
        "cycle" => "a value that contains itself".to_string(),
        "object" => "an object that isn't a plain object or an array".to_string(),
        _ => match kind.strip_prefix("object:") {
            Some(name) => format!("a `{name}` object, which isn't a plain object"),
            None => format!("a value of kind `{kind}`"),
        },
    }
}
