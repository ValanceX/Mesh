//! The types analysis works with: every type a manifest can write, plus
//! the two internal types (`nothing` and `void`), in one representation.

use std::collections::BTreeMap;
use std::fmt;

/// A type, as analysis computes and compares them.
///
/// The variants a manifest can write mirror [`mesh_manifest::Type`]
/// (convert with `Ty::from`); [`Ty::Nothing`] and [`Ty::Void`] exist only
/// inside analysis. Named references stay [`Ty::Named`], so a type prints
/// the way the manifest wrote it; [`crate::is_assignable`] and
/// [`crate::join`] expand them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    String,
    Number,
    Boolean,
    /// The value `null`. Not absence: that is [`Ty::Optional`].
    Null,
    /// Any present value.
    Any,
    /// `list<T>`.
    List(Box<Ty>),
    /// An exact record: these fields, and no others.
    Record(BTreeMap<String, FieldTy>),
    /// A reference to one of the manifest's named types.
    Named(String),
    /// `T?`: a `T`, or absent. Never wraps a type that is already
    /// optional, directly or through named references.
    Optional(Box<Ty>),
    /// The element type of `[]`. Assignable to every type.
    Nothing,
    /// The result of a command invocation. Compatible with nothing.
    Void,
}

/// A record field's declaration: its type, and whether it must be
/// supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldTy {
    pub ty: Ty,
    pub required: bool,
}

impl From<&mesh_manifest::Type> for Ty {
    fn from(ty: &mesh_manifest::Type) -> Self {
        use mesh_manifest::Type;
        match ty {
            Type::String => Ty::String,
            Type::Number => Ty::Number,
            Type::Boolean => Ty::Boolean,
            Type::Null => Ty::Null,
            Type::Any => Ty::Any,
            Type::List(element) => Ty::List(Box::new(Ty::from(element.as_ref()))),
            Type::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(name, field)| {
                        (
                            name.clone(),
                            FieldTy {
                                ty: Ty::from(&field.ty),
                                required: field.required,
                            },
                        )
                    })
                    .collect(),
            ),
            Type::Named(name) => Ty::Named(name.clone()),
            Type::Optional(inner) => Ty::Optional(Box::new(Ty::from(inner.as_ref()))),
        }
    }
}

/// The notation the language docs use: `string`, `list<User>`, `boolean?`,
/// `{ name: string, avatar?: string }`. A named type prints as its name.
impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::String => f.write_str("string"),
            Ty::Number => f.write_str("number"),
            Ty::Boolean => f.write_str("boolean"),
            Ty::Null => f.write_str("null"),
            Ty::Any => f.write_str("any"),
            Ty::Nothing => f.write_str("nothing"),
            Ty::Void => f.write_str("void"),
            Ty::List(element) => write!(f, "list<{element}>"),
            Ty::Named(name) => f.write_str(name),
            Ty::Optional(inner) => write!(f, "{inner}?"),
            Ty::Record(fields) if fields.is_empty() => f.write_str("{}"),
            Ty::Record(fields) => {
                f.write_str("{ ")?;
                for (index, (name, field)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    let mark = if field.required { "" } else { "?" };
                    write!(f, "{name}{mark}: {}", field.ty)?;
                }
                f.write_str(" }")
            }
        }
    }
}
