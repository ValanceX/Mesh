//! Types at runtime: *fits*, the one relation every runtime type check
//! uses (§9.7.2), and the little static typing member access needs.

use crate::value::Value;
use mesh_manifest::{Manifest, Type};
use mesh_template::Expression;
use std::collections::BTreeMap;

/// §9.7.2's *fits(value, T)*. Named types are expanded first.
pub(crate) fn fits(manifest: &Manifest, value: &Value, ty: &Type) -> bool {
    match (manifest.expand(ty), value) {
        (Type::Optional(_), Value::Absent) => true,
        (Type::Optional(inner), _) => fits(manifest, value, inner),
        (_, Value::Absent) => false,
        (Type::Any, _) => true,
        (Type::String, Value::String(_))
        | (Type::Number, Value::Number(_))
        | (Type::Boolean, Value::Boolean(_))
        | (Type::Null, Value::Null) => true,
        (Type::List(element), Value::List(items)) => {
            items.iter().all(|item| fits(manifest, item, element))
        }
        (Type::Record(fields), Value::Record(record)) => {
            record.keys().all(|name| fields.contains_key(name))
                && fields.iter().all(|(name, field)| match record.get(name) {
                    Some(value) => fits(manifest, value, &field.ty),
                    None => reads_as_optional(manifest, field),
                })
        }
        _ => false,
    }
}

/// Whether a field reads as optional (§9.2): not required, or of an
/// optional type.
pub(crate) fn reads_as_optional(manifest: &Manifest, field: &mesh_manifest::Field) -> bool {
    !field.required || matches!(manifest.expand(&field.ty), Type::Optional(_))
}

/// An expression's static type, as far as member access needs it
/// (§9.7.6): on a value of a record type, an absent field reads as
/// absent, but on `any` the field must be present. A template holds no
/// types, so this recovers from the model just whether a member access's
/// object is `any`, and a record's field types for a deeper access.
#[derive(Debug, Clone)]
pub(crate) enum Static<'m> {
    Any,
    /// A declared type from the model.
    Declared(&'m Type),
    /// A record literal's fields.
    Literal(BTreeMap<String, Static<'m>>),
    /// Anything a member access can't statically be made on.
    Other,
}

/// What an expression's static type depends on: the template's scope
/// types, and the payload type of the handler's event, if in one.
pub(crate) struct Statics<'m> {
    pub manifest: &'m Manifest,
    pub scope: &'m BTreeMap<String, Type>,
    pub payload: Option<&'m Type>,
}

impl<'m> Statics<'m> {
    pub(crate) fn is_any(&self, st: &Static<'m>) -> bool {
        match st {
            Static::Any => true,
            Static::Declared(ty) => match self.manifest.expand(ty) {
                Type::Any => true,
                Type::Optional(inner) => matches!(self.manifest.expand(inner), Type::Any),
                _ => false,
            },
            Static::Literal(_) | Static::Other => false,
        }
    }

    fn field(&self, st: Static<'m>, name: &str) -> Static<'m> {
        match st {
            Static::Any => Static::Any,
            Static::Declared(ty) => {
                let mut ty = self.manifest.expand(ty);
                if let Type::Optional(inner) = ty {
                    ty = self.manifest.expand(inner);
                }
                match ty {
                    Type::Any => Static::Any,
                    Type::Record(fields) => fields
                        .get(name)
                        .map_or(Static::Other, |field| Static::Declared(&field.ty)),
                    _ => Static::Other,
                }
            }
            Static::Literal(mut fields) => fields.remove(name).unwrap_or(Static::Other),
            Static::Other => Static::Other,
        }
    }

    /// The static type of `expression`, as far as [`Static`] tells.
    pub(crate) fn of(&self, expression: &Expression) -> Static<'m> {
        match expression {
            Expression::Scope { name, .. } => {
                self.scope.get(name).map_or(Static::Other, Static::Declared)
            }
            Expression::Member { object, field, .. } => self.field(self.of(object), field),
            Expression::Conditional {
                consequent,
                alternate,
                ..
            } => {
                // The branches' join: `any` absorbs; otherwise they're the
                // same record type (§9.3), so either one's will do.
                let (a, b) = (self.of(consequent), self.of(alternate));
                if self.is_any(&a) || self.is_any(&b) {
                    Static::Any
                } else {
                    a
                }
            }
            Expression::Record { fields, .. } => Static::Literal(
                fields
                    .iter()
                    .map(|field| (field.name.clone(), self.of(&field.value)))
                    .collect(),
            ),
            Expression::Event { .. } => self.payload.map_or(Static::Other, Static::Declared),
            Expression::Literal { .. }
            | Expression::Unary { .. }
            | Expression::Binary { .. }
            | Expression::List { .. } => Static::Other,
        }
    }
}
