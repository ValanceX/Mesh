//! `members`: the one rule for which members a type has, shared by the
//! checker and an editor's completion.

use mesh_analysis::{members, read_field, FieldTy, Ty};
use std::collections::BTreeMap;

const MANIFEST: &str = include_str!("../../../examples/components.json");

fn manifest() -> mesh_manifest::Manifest {
    mesh_manifest::load(MANIFEST).expect("the example manifest loads")
}

fn names(fields: Option<BTreeMap<String, FieldTy>>) -> Option<Vec<String>> {
    fields.map(|fields| fields.into_keys().collect())
}

#[test]
fn a_named_record_has_its_fields() {
    let manifest = manifest();
    let user = Ty::Named("User".to_string());
    assert_eq!(
        names(members(&manifest, &user)),
        Some(vec![
            "active".to_string(),
            "avatar".to_string(),
            "name".to_string()
        ])
    );
    let fields = members(&manifest, &user).unwrap();
    assert_eq!(read_field(&manifest, &fields["name"]), Ty::String);
    assert_eq!(
        read_field(&manifest, &fields["avatar"]),
        Ty::Optional(Box::new(Ty::String))
    );
}

#[test]
fn an_anonymous_record_has_its_fields() {
    let manifest = manifest();
    let record = Ty::Record(BTreeMap::from([(
        "x".to_string(),
        FieldTy {
            ty: Ty::Number,
            required: true,
        },
    )]));
    assert_eq!(
        names(members(&manifest, &record)),
        Some(vec!["x".to_string()])
    );
}

#[test]
fn nothing_else_has_members() {
    let manifest = manifest();
    let user = Ty::Named("User".to_string());
    for ty in [
        Ty::Optional(Box::new(user.clone())),
        Ty::Any,
        Ty::List(Box::new(user)),
        Ty::String,
        Ty::Number,
        Ty::Null,
        Ty::Void,
    ] {
        assert_eq!(members(&manifest, &ty), None, "{ty}");
    }
}
