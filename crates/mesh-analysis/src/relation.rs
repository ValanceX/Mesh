//! The one compatibility relation, `is_assignable`, and the common type,
//! `join` (outline D9). Every check that compares types goes through
//! these two functions; no checker has compatibility logic of its own.

use crate::types::{FieldTy, Ty};
use mesh_manifest::Manifest;
use std::collections::BTreeMap;

/// `ty` with named references at its top level followed to their
/// definition. The result is never [`Ty::Named`].
pub(crate) fn expand(manifest: &Manifest, ty: &Ty) -> Ty {
    let mut ty = ty.clone();
    while let Ty::Named(name) = &ty {
        ty = Ty::from(&manifest.types()[name]);
    }
    ty
}

/// `ty?`, unless `ty` is already optional (directly or through named
/// references), in which case `ty` itself: optionality never nests.
pub(crate) fn optional(manifest: &Manifest, ty: Ty) -> Ty {
    match expand(manifest, &ty) {
        Ty::Optional(_) => ty,
        _ => Ty::Optional(Box::new(ty)),
    }
}

/// The type a field's value has when it's read (outline D5: requiredness
/// is erased on read). `f: T` reads as `T`; `f?: T`, `f: T?` and `f?: T?`
/// all read as `T?`.
pub(crate) fn read(manifest: &Manifest, field: &FieldTy) -> Ty {
    if field.required {
        field.ty.clone()
    } else {
        optional(manifest, field.ty.clone())
    }
}

/// The members a value of type `ty` has, by name, if it's a record once
/// named references at its top level are expanded; `None` for every other
/// type. This is the only rule for which members exist: the checker reads
/// a member through it, and an editor offers its keys. An optional type
/// has none (reading one is `possibly-absent-access`), and `any` has no
/// list: every member of it is allowed and unknown.
///
/// A member's value, when read, has the type [`read_field`] gives.
pub fn members(manifest: &Manifest, ty: &Ty) -> Option<BTreeMap<String, FieldTy>> {
    match expand(manifest, ty) {
        Ty::Record(fields) => Some(fields),
        _ => None,
    }
}

/// The type a record field's value has when it's read: requiredness is
/// erased on read, so an optional field of type `T` reads as `T?`.
pub fn read_field(manifest: &Manifest, field: &FieldTy) -> Ty {
    read(manifest, field)
}

/// Whether a value of type `actual` may be used where `expected` is
/// expected: outline D9's `is_assignable`, rule by rule. Named references
/// are expanded first, and the first matching rule decides.
///
/// `manifest` must be the manifest the types came from, so that every
/// named reference in them resolves.
pub fn is_assignable(manifest: &Manifest, actual: &Ty, expected: &Ty) -> bool {
    let actual = expand(manifest, actual);
    let expected = expand(manifest, expected);
    match (&actual, &expected) {
        // 1. `void` is compatible with nothing, not even `any`.
        (Ty::Void, _) | (_, Ty::Void) => false,
        // 2. Optionality, for every type, `any` included. A present value
        //    satisfies "may be absent"; a possibly absent one satisfies
        //    only that.
        (Ty::Optional(actual), Ty::Optional(expected)) => is_assignable(manifest, actual, expected),
        (_, Ty::Optional(expected)) => is_assignable(manifest, &actual, expected),
        (Ty::Optional(_), _) => false,
        // 3, 4. `any` is unchecked against the value's type, either way.
        (_, Ty::Any) | (Ty::Any, _) => true,
        // 5. `nothing` (the elements of `[]`) fits everywhere.
        (Ty::Nothing, _) => true,
        // 6. Primitives: the same type, and no coercions.
        (Ty::String, Ty::String)
        | (Ty::Number, Ty::Number)
        | (Ty::Boolean, Ty::Boolean)
        | (Ty::Null, Ty::Null) => true,
        // 7. Lists are covariant: MPRX values are never mutated.
        (Ty::List(actual), Ty::List(expected)) => is_assignable(manifest, actual, expected),
        // 8. Records compare declarations, requiredness included, and are
        //    exact.
        (Ty::Record(actual), Ty::Record(expected)) => {
            let fits = expected.iter().all(|(name, expected)| {
                match (actual.get(name), expected.required) {
                    (Some(actual), true) => {
                        actual.required && is_assignable(manifest, &actual.ty, &expected.ty)
                    }
                    (Some(actual), false) => is_assignable(manifest, &actual.ty, &expected.ty),
                    (None, required) => !required,
                }
            });
            fits && actual.keys().all(|name| expected.contains_key(name))
        }
        // 9. Anything else.
        _ => false,
    }
}

/// The common type of `a` and `b`, if they have one: outline D9's `join`,
/// used for conditional branches, array elements and equality operands.
/// Named references are expanded first, and the first matching rule
/// decides. Where the result is one of the arguments, it is returned as
/// written, so a named type keeps its name.
///
/// `manifest` must be the manifest the types came from.
pub fn join(manifest: &Manifest, a: &Ty, b: &Ty) -> Option<Ty> {
    let expanded_a = expand(manifest, a);
    let expanded_b = expand(manifest, b);
    match (&expanded_a, &expanded_b) {
        // 1. Nothing joins with `void`.
        (Ty::Void, _) | (_, Ty::Void) => None,
        // 2. Optionality is never discarded, `any` included.
        (Ty::Optional(a), Ty::Optional(b)) => Some(optional(manifest, join(manifest, a, b)?)),
        (Ty::Optional(a), _) => Some(optional(manifest, join(manifest, a, &expanded_b)?)),
        (_, Ty::Optional(b)) => Some(optional(manifest, join(manifest, &expanded_a, b)?)),
        // 3. `nothing` adds nothing.
        (Ty::Nothing, _) => Some(b.clone()),
        (_, Ty::Nothing) => Some(a.clone()),
        // 4. `any` absorbs every present type.
        (Ty::Any, _) | (_, Ty::Any) => Some(Ty::Any),
        // 5. Structurally identical types.
        _ if identical(manifest, &expanded_a, &expanded_b) => Some(a.clone()),
        // 6. Lists join element-wise.
        (Ty::List(a), Ty::List(b)) => Some(Ty::List(Box::new(join(manifest, a, b)?))),
        // 7. Anything else.
        _ => None,
    }
}

/// Whether `a` and `b` are the same type once every named reference in
/// them is expanded. Records must declare the same fields, each with the
/// same requiredness and an identical type.
fn identical(manifest: &Manifest, a: &Ty, b: &Ty) -> bool {
    match (expand(manifest, a), expand(manifest, b)) {
        (Ty::List(a), Ty::List(b)) | (Ty::Optional(a), Ty::Optional(b)) => {
            identical(manifest, &a, &b)
        }
        (Ty::Record(a), Ty::Record(b)) => {
            a.len() == b.len()
                && a.iter().all(|(name, a)| {
                    b.get(name).is_some_and(|b| {
                        a.required == b.required && identical(manifest, &a.ty, &b.ty)
                    })
                })
        }
        (a, b) => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Maybe` is `string?`, and `Alias` is `Maybe` through one more name.
    fn manifest() -> Manifest {
        mesh_manifest::load(
            r#"{ "version": 1, "components": {}, "types": {
                "Maybe": { "kind": "optional", "type": { "kind": "string" } },
                "Alias": { "kind": "named", "name": "Maybe" },
                "Name": { "kind": "string" }
            } }"#,
        )
        .expect("the test manifest loads")
    }

    fn named(name: &str) -> Ty {
        Ty::Named(name.to_string())
    }

    fn opt(ty: Ty) -> Ty {
        Ty::Optional(Box::new(ty))
    }

    #[test]
    fn expands_named_references_at_the_top_level_only() {
        let manifest = manifest();
        assert_eq!(expand(&manifest, &named("Alias")), opt(Ty::String));
        let list = Ty::List(Box::new(named("Name")));
        assert_eq!(expand(&manifest, &list), list);
    }

    #[test]
    fn optionality_never_nests() {
        let manifest = manifest();
        assert_eq!(optional(&manifest, Ty::String), opt(Ty::String));
        assert_eq!(optional(&manifest, opt(Ty::String)), opt(Ty::String));
        assert_eq!(optional(&manifest, named("Alias")), named("Alias"));
        assert_eq!(optional(&manifest, named("Name")), opt(named("Name")));
    }

    #[test]
    fn reading_a_field_erases_requiredness() {
        let manifest = manifest();
        let field = |ty: Ty, required: bool| FieldTy { ty, required };
        assert_eq!(read(&manifest, &field(Ty::String, true)), Ty::String);
        assert_eq!(read(&manifest, &field(Ty::String, false)), opt(Ty::String));
        assert_eq!(
            read(&manifest, &field(opt(Ty::String), true)),
            opt(Ty::String)
        );
        assert_eq!(
            read(&manifest, &field(opt(Ty::String), false)),
            opt(Ty::String)
        );
        assert_eq!(
            read(&manifest, &field(named("Maybe"), false)),
            named("Maybe")
        );
    }

    #[test]
    fn named_types_are_aliases() {
        let manifest = manifest();
        assert!(is_assignable(&manifest, &named("Name"), &Ty::String));
        assert!(is_assignable(&manifest, &Ty::String, &named("Name")));
        assert!(is_assignable(
            &manifest,
            &named("Alias"),
            &opt(named("Name"))
        ));
        assert!(!is_assignable(&manifest, &named("Alias"), &Ty::String));
        assert_eq!(
            join(&manifest, &named("Name"), &Ty::String),
            Some(named("Name"))
        );
        assert_eq!(join(&manifest, &named("Alias"), &Ty::Null), None);
    }

    #[test]
    fn joins_keep_a_named_type_optional_through_its_alias() {
        let manifest = manifest();
        assert_eq!(
            join(&manifest, &named("Alias"), &Ty::String),
            Some(opt(Ty::String))
        );
        assert_eq!(
            join(&manifest, &named("Alias"), &Ty::Nothing),
            Some(opt(Ty::String))
        );
    }
}
