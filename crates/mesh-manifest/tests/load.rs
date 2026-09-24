use mesh_manifest::{load, Type};
use mesh_syntax::{Diagnostic, DiagnosticCode, Span};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn example() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/components.json");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// Loads `source`, expecting it to fail, and returns each diagnostic's
/// code and the text its span covers.
fn errors(source: &str) -> Vec<(DiagnosticCode, &str)> {
    let diagnostics = load(source).expect_err("the manifest should be rejected");
    diagnostics
        .iter()
        .map(|d| (d.code, &source[d.span.start_byte..d.span.end_byte]))
        .collect()
}

/// A version 1 manifest with these `types` and `components` (JSON text).
fn manifest(types: &str, components: &str) -> String {
    format!(r#"{{ "version": 1, "types": {types}, "components": {components} }}"#)
}

/// A manifest with one component, `c`, whose parts are these JSON texts.
fn component(props: &str, events: &str, commands: &str, scope: &str) -> String {
    manifest(
        "{}",
        &format!(
            r#"{{ "c": {{ "props": {props}, "events": {events}, "commands": {commands}, "scope": {scope} }} }}"#
        ),
    )
}

/// A manifest whose component `c` has one scope name, `x`, of this type.
fn scope_type(ty: &str) -> String {
    component("{}", "{}", "{}", &format!(r#"{{ "x": {ty} }}"#))
}

#[test]
fn loads_the_example_manifest() {
    let manifest = load(&example()).expect("the example manifest should load");

    let names: Vec<&str> = manifest.components().keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "avatar",
            "button",
            "page",
            "text",
            "user-card",
            "user-card-example",
            "users-page"
        ]
    );
    let card = &manifest.components()["user-card"];
    assert_eq!(card.props["user"].ty, Type::Named("User".to_string()));
    assert!(card.props["user"].required);
    assert_eq!(
        card.props["compact"].ty,
        Type::Optional(Box::new(Type::Boolean))
    );
    assert!(!card.props["compact"].required);
    assert_eq!(
        card.events["select"].payload,
        Some(Type::Named("User".to_string()))
    );

    let page = &manifest.components()["users-page"];
    let select = &page.commands["selectUser"].parameters;
    assert_eq!(select.len(), 1);
    assert_eq!(select[0].name, "user");
    assert_eq!(page.scope["compact"], Type::Boolean);

    let Type::Record(user) = manifest.expand(&page.scope["user"]) else {
        panic!("User should expand to a record");
    };
    assert_eq!(user["avatar"].ty, Type::String);
    assert!(!user["avatar"].required);
}

#[test]
fn reads_every_kind() {
    let source = manifest(
        r#"{ "Alias": { "kind": "named", "name": "Target" }, "Target": { "kind": "list", "element": { "kind": "any" } } }"#,
        r#"{ "c": { "props": {}, "events": { "e": {} }, "commands": { "go": { "parameters": [] } }, "scope": {
            "s": { "kind": "string" }, "n": { "kind": "number" }, "b": { "kind": "boolean" },
            "z": { "kind": "null" }, "o": { "kind": "optional", "type": { "kind": "named", "name": "Alias" } },
            "r": { "kind": "record", "fields": {} }
        } } }"#,
    );
    let manifest = load(&source).expect("should load");
    let c = &manifest.components()["c"];
    assert_eq!(c.scope["s"], Type::String);
    assert_eq!(c.scope["n"], Type::Number);
    assert_eq!(c.scope["b"], Type::Boolean);
    assert_eq!(c.scope["z"], Type::Null);
    assert_eq!(c.scope["r"], Type::Record(BTreeMap::new()));
    assert_eq!(c.events["e"].payload, None);
    assert!(c.commands["go"].parameters.is_empty());
    let Type::Optional(inner) = &c.scope["o"] else {
        panic!("expected an optional type");
    };
    // Expanding follows a chain of aliases to the definition.
    assert_eq!(manifest.expand(inner), &Type::List(Box::new(Type::Any)));
}

#[test]
fn allows_a_byte_order_mark() {
    let source = format!("\u{feff}{}", manifest("{}", "{}"));
    assert!(load(&source).is_ok());
}

#[test]
fn rejects_invalid_json_at_the_mistake() {
    let source = "{\n  \"version\": 1,\n  \"types\": {},\n  \"components\": {}\n  \"extra\": 1\n}";
    let diagnostics = load(source).expect_err("invalid JSON");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::MANIFEST_SYNTAX_ERROR);
    assert_eq!(
        diagnostics[0].message,
        "the manifest isn't valid JSON: expected `,` or `}`"
    );
    assert_eq!(&source[diagnostics[0].span.start_byte..][..7], "\"extra\"");
}

#[test]
fn checks_the_version_before_anything_else() {
    assert_eq!(
        errors(r#"{ "version": 2, "bogus": 1 }"#),
        [(DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION, "2")]
    );
    assert_eq!(
        errors(r#"{ "version": "1" }"#),
        [(DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION, r#""1""#)]
    );
    assert_eq!(
        errors(r#"{ "types": {} }"#),
        [(DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION, "{")]
    );
    assert_eq!(
        errors("[]"),
        [(DiagnosticCode::MANIFEST_INVALID_VALUE, "[]")]
    );
}

#[test]
fn rejects_missing_and_unknown_properties() {
    assert_eq!(
        errors(r#"{ "version": 1, "types": {}, "extra": true }"#),
        [
            (DiagnosticCode::MANIFEST_MISSING_PROPERTY, "{"),
            (DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY, r#""extra""#),
        ]
    );
    // A field declaration must say whether it is required.
    assert_eq!(
        errors(&component(
            r#"{ "p": { "type": { "kind": "string" } } }"#,
            "{}",
            "{}",
            "{}"
        )),
        [(DiagnosticCode::MANIFEST_MISSING_PROPERTY, "{")]
    );
    assert_eq!(
        errors(&scope_type(
            r#"{ "kind": "string", "element": { "kind": "any" } }"#
        )),
        [(DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY, r#""element""#)]
    );
    assert_eq!(
        errors(&scope_type(r#"{ "kind": "list" }"#)),
        [(DiagnosticCode::MANIFEST_MISSING_PROPERTY, "{")]
    );
}

#[test]
fn rejects_values_of_the_wrong_json_type() {
    assert_eq!(
        errors(&component(
            r#"{ "p": { "type": { "kind": "string" }, "required": "yes" } }"#,
            "{}",
            "[]",
            "{}"
        )),
        [
            (DiagnosticCode::MANIFEST_INVALID_VALUE, r#""yes""#),
            (DiagnosticCode::MANIFEST_INVALID_VALUE, "[]"),
        ]
    );
    assert_eq!(
        errors(&scope_type(r#""string""#)),
        [(DiagnosticCode::MANIFEST_INVALID_VALUE, r#""string""#)]
    );
}

#[test]
fn rejects_unknown_kinds_including_internal_ones() {
    let source = scope_type(r#"{ "kind": "void" }"#);
    let diagnostics = load(&source).expect_err("void is internal");
    assert_eq!(diagnostics[0].code, DiagnosticCode::MANIFEST_UNKNOWN_KIND);
    assert_eq!(
        diagnostics[0].message,
        "`void` is internal to MESH; a manifest can't write it"
    );
    assert_eq!(
        errors(&scope_type(r#"{ "kind": "nothing" }"#)),
        [(DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""nothing""#)]
    );
    assert_eq!(
        errors(&scope_type(r#"{ "kind": "union" }"#)),
        [(DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""union""#)]
    );
}

/// JSON parsers usually keep the last of two equal keys. MESH reports
/// the repeat, at every level.
#[test]
fn rejects_duplicate_keys_at_every_level() {
    let cases = [
        manifest(
            "{}",
            r#"{ "c": { "props": {}, "events": {}, "commands": {}, "scope": {} }, "c": { "props": {}, "events": {}, "commands": {}, "scope": {} } }"#,
        ),
        component(
            r#"{ "p": { "type": { "kind": "any" }, "required": true }, "p": { "type": { "kind": "any" }, "required": true } }"#,
            "{}",
            "{}",
            "{}",
        ),
        component("{}", r#"{ "e": {}, "e": {} }"#, "{}", "{}"),
        component(
            "{}",
            "{}",
            r#"{ "go": { "parameters": [] }, "go": { "parameters": [] } }"#,
            "{}",
        ),
        component(
            "{}",
            "{}",
            "{}",
            r#"{ "x": { "kind": "any" }, "x": { "kind": "any" } }"#,
        ),
        manifest(
            r#"{ "T": { "kind": "any" }, "T": { "kind": "any" } }"#,
            "{}",
        ),
        scope_type(
            r#"{ "kind": "record", "fields": { "f": { "type": { "kind": "any" }, "required": true }, "f": { "type": { "kind": "any" }, "required": true } } }"#,
        ),
        scope_type(r#"{ "kind": "any", "kind": "any" }"#),
    ];
    for source in &cases {
        let found = errors(source);
        assert_eq!(found.len(), 1, "{source}: {found:?}");
        assert_eq!(
            found[0].0,
            DiagnosticCode::MANIFEST_DUPLICATE_KEY,
            "{source}"
        );
        // The second occurrence is reported.
        let second = source.rfind(found[0].1).expect("the key is in the source");
        assert!(source.find(found[0].1) < Some(second), "{source}");
    }
}

#[test]
fn rejects_duplicate_parameter_names() {
    assert_eq!(
        errors(&component(
            "{}",
            "{}",
            r#"{ "go": { "parameters": [
                { "name": "a", "type": { "kind": "any" } },
                { "name": "a", "type": { "kind": "any" } }
            ] } }"#,
            "{}"
        )),
        [(DiagnosticCode::MANIFEST_DUPLICATE_PARAMETER, r#""a""#)]
    );
}

#[test]
fn rejects_names_mprx_cannot_write() {
    let cases = [
        (
            manifest(
                "{}",
                r#"{ "1card": { "props": {}, "events": {}, "commands": {}, "scope": {} } }"#,
            ),
            r#""1card""#,
        ),
        (
            manifest(
                "{}",
                r#"{ "user card": { "props": {}, "events": {}, "commands": {}, "scope": {} } }"#,
            ),
            r#""user card""#,
        ),
        (
            component(
                r#"{ "data-id": { "type": { "kind": "any" }, "required": true } }"#,
                "{}",
                "{}",
                "{}",
            ),
            r#""data-id""#,
        ),
        (
            component("{}", r#"{ "on.click": {} }"#, "{}", "{}"),
            r#""on.click""#,
        ),
        (
            component("{}", "{}", r#"{ "go-now": { "parameters": [] } }"#, "{}"),
            r#""go-now""#,
        ),
        (
            component(
                "{}",
                "{}",
                r#"{ "go": { "parameters": [{ "name": "", "type": { "kind": "any" } }] } }"#,
                "{}",
            ),
            r#""""#,
        ),
        (
            component("{}", "{}", "{}", r#"{ "null": { "kind": "any" } }"#),
            r#""null""#,
        ),
        (
            manifest(r#"{ "My-Type": { "kind": "any" } }"#, "{}"),
            r#""My-Type""#,
        ),
        (
            scope_type(
                r#"{ "kind": "record", "fields": { "true": { "type": { "kind": "any" }, "required": true } } }"#,
            ),
            r#""true""#,
        ),
    ];
    for (source, name) in &cases {
        assert_eq!(
            errors(source),
            [(DiagnosticCode::MANIFEST_INVALID_NAME, *name)],
            "{source}"
        );
    }
    // Hyphens are fine in a component (tag) name.
    assert!(load(&manifest(
        "{}",
        r#"{ "user-card": { "props": {}, "events": {}, "commands": {}, "scope": {} } }"#
    ))
    .is_ok());
}

#[test]
fn rejects_unknown_named_types() {
    assert_eq!(
        errors(&scope_type(r#"{ "kind": "named", "name": "Usr" }"#)),
        [(DiagnosticCode::MANIFEST_UNKNOWN_TYPE, r#""Usr""#)]
    );
}

#[test]
fn rejects_recursive_types_once_per_cycle() {
    let source = manifest(
        r#"{
            "Leaf": { "kind": "list", "element": { "kind": "named", "name": "Node" } },
            "Node": { "kind": "record", "fields": { "children": { "type": { "kind": "named", "name": "Children" }, "required": true } } },
            "Children": { "kind": "list", "element": { "kind": "named", "name": "Node" } },
            "Me": { "kind": "optional", "type": { "kind": "named", "name": "Me" } }
        }"#,
        "{}",
    );
    let diagnostics = load(&source).expect_err("recursive types");
    let found: Vec<(DiagnosticCode, &str, &str)> = diagnostics
        .iter()
        .map(|d| {
            (
                d.code,
                &source[d.span.start_byte..d.span.end_byte],
                d.message.as_str(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                DiagnosticCode::MANIFEST_RECURSIVE_TYPE,
                r#""Node""#,
                r#"type "Node" refers to itself: Node -> Children -> Node"#
            ),
            (
                DiagnosticCode::MANIFEST_RECURSIVE_TYPE,
                r#""Me""#,
                r#"type "Me" refers to itself: Me -> Me"#
            ),
        ]
    );
}

#[test]
fn rejects_an_optional_wrapping_an_optional() {
    assert_eq!(
        errors(&scope_type(
            r#"{ "kind": "optional", "type": { "kind": "optional", "type": { "kind": "any" } } }"#
        )),
        [(
            DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
            r#"{ "kind": "optional", "type": { "kind": "any" } }"#
        )]
    );
    // An error inside the inner type takes precedence: it is reported,
    // and the nesting isn't.
    assert_eq!(
        errors(&scope_type(
            r#"{ "kind": "optional", "type": { "kind": "optional", "type": { "kind": "void" } } }"#
        )),
        [(DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""void""#)]
    );
    // Also through named types, which are aliases.
    let source = manifest(
        r#"{ "Maybe": { "kind": "optional", "type": { "kind": "string" } }, "Alias": { "kind": "named", "name": "Maybe" } }"#,
        r#"{ "c": { "props": {}, "events": {}, "commands": {}, "scope": { "x": { "kind": "optional", "type": { "kind": "named", "name": "Alias" } } } } }"#,
    );
    assert_eq!(
        errors(&source),
        [(DiagnosticCode::MANIFEST_NESTED_OPTIONAL, r#""Alias""#)]
    );
}

#[test]
fn reports_every_problem_in_source_order() {
    let source = manifest(
        r#"{ "T": { "kind": "void" } }"#,
        r#"{ "c": { "props": { "bad-name": { "type": { "kind": "named", "name": "Nope" }, "required": 1 } }, "events": {}, "commands": {}, "scope": {} } }"#,
    );
    let codes: Vec<DiagnosticCode> = errors(&source).into_iter().map(|(code, _)| code).collect();
    assert_eq!(
        codes,
        [
            DiagnosticCode::MANIFEST_UNKNOWN_KIND,
            DiagnosticCode::MANIFEST_INVALID_NAME,
            DiagnosticCode::MANIFEST_UNKNOWN_TYPE,
            DiagnosticCode::MANIFEST_INVALID_VALUE,
        ]
    );
}

#[test]
fn a_template_is_a_declared_component() {
    let source = example();
    let manifest = load(&source).expect("should load");

    let template = manifest.template("users-page").expect("declared");
    assert_eq!(template.name(), "users-page");
    assert!(template.component().scope.contains_key("user"));

    let Diagnostic {
        code,
        message,
        span,
        ..
    } = manifest.template("user-list").expect_err("not declared");
    assert_eq!(code, DiagnosticCode::MANIFEST_MISSING_COMPONENT);
    assert_eq!(
        message,
        r#"the manifest declares no component "user-list", which this file is the template of"#
    );
    assert_eq!(&source[span.start_byte..span.end_byte], r#""components""#);
    let _: Span = span;
}

/// `true`, `false` and `null` are always literals to MPRX's lexer, so no
/// name that MPRX reads as an identifier can be one of them.
#[test]
fn reserved_words_are_rejected_in_every_identifier_position() {
    for word in ["true", "false", "null"] {
        let any = r#"{ "kind": "any" }"#;
        let cases = [
            (
                "prop",
                component(
                    &format!(r#"{{ "{word}": {{ "type": {any}, "required": true }} }}"#),
                    "{}",
                    "{}",
                    "{}",
                ),
            ),
            (
                "event",
                component("{}", &format!(r#"{{ "{word}": {{}} }}"#), "{}", "{}"),
            ),
            (
                "command",
                component(
                    "{}",
                    "{}",
                    &format!(r#"{{ "{word}": {{ "parameters": [] }} }}"#),
                    "{}",
                ),
            ),
            (
                "parameter",
                component(
                    "{}",
                    "{}",
                    &format!(
                        r#"{{ "go": {{ "parameters": [{{ "name": "{word}", "type": {any} }}] }} }}"#
                    ),
                    "{}",
                ),
            ),
            (
                "scope name",
                component("{}", "{}", "{}", &format!(r#"{{ "{word}": {any} }}"#)),
            ),
            ("type", manifest(&format!(r#"{{ "{word}": {any} }}"#), "{}")),
            (
                "field",
                scope_type(&format!(
                    r#"{{ "kind": "record", "fields": {{ "{word}": {{ "type": {any}, "required": true }} }} }}"#
                )),
            ),
        ];
        for (noun, source) in &cases {
            let diagnostics = load(source).expect_err("a reserved name");
            assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
            let d = &diagnostics[0];
            assert_eq!(d.code, DiagnosticCode::MANIFEST_INVALID_NAME, "{source}");
            assert_eq!(
                &source[d.span.start_byte..d.span.end_byte],
                format!("\"{word}\"")
            );
            assert_eq!(
                d.message,
                format!("{noun} name \"{word}\" is reserved: MPRX reads it as a literal")
            );
        }
    }
    // Only the exact words are reserved.
    assert!(load(&component(
        "{}",
        "{}",
        "{}",
        r#"{ "nullable": { "kind": "any" }, "True": { "kind": "any" }, "_null": { "kind": "any" } }"#
    ))
    .is_ok());
}

#[test]
fn rejects_an_optional_alias_through_a_chain_of_aliases() {
    let source = manifest(
        r#"{
            "A": { "kind": "named", "name": "B" },
            "B": { "kind": "named", "name": "C" },
            "C": { "kind": "optional", "type": { "kind": "string" } },
            "D": { "kind": "optional", "type": { "kind": "named", "name": "A" } }
        }"#,
        r#"{ "c": { "props": {}, "events": {}, "commands": {}, "scope": {
            "x": { "kind": "optional", "type": { "kind": "named", "name": "B" } }
        } } }"#,
    );
    // Inside a named type's definition, and in a declaration.
    assert_eq!(
        errors(&source),
        [
            (DiagnosticCode::MANIFEST_NESTED_OPTIONAL, r#""A""#),
            (DiagnosticCode::MANIFEST_NESTED_OPTIONAL, r#""B""#),
        ]
    );
}

/// An alias that *contains* an optional somewhere inside it isn't itself
/// optional, so wrapping it in `optional` is fine. Only an alias that
/// *expands to* an optional type can't be wrapped again.
#[test]
fn accepts_optional_aliases_where_nothing_is_doubly_optional() {
    let source = manifest(
        r#"{
            "Maybe": { "kind": "optional", "type": { "kind": "string" } },
            "Names": { "kind": "list", "element": { "kind": "named", "name": "Maybe" } },
            "Person": { "kind": "record", "fields": {
                "nick": { "type": { "kind": "named", "name": "Maybe" }, "required": false }
            } },
            "Chain": { "kind": "named", "name": "Person" }
        }"#,
        r#"{ "c": { "props": {
            "p": { "type": { "kind": "named", "name": "Maybe" }, "required": false }
        }, "events": {}, "commands": {}, "scope": {
            "a": { "kind": "list", "element": { "kind": "named", "name": "Maybe" } },
            "b": { "kind": "optional", "type": { "kind": "list", "element": { "kind": "named", "name": "Maybe" } } },
            "c": { "kind": "optional", "type": { "kind": "named", "name": "Person" } },
            "d": { "kind": "optional", "type": { "kind": "named", "name": "Chain" } },
            "e": { "kind": "named", "name": "Maybe" },
            "f": { "kind": "optional", "type": { "kind": "named", "name": "Names" } }
        } } }"#,
    );
    let manifest = load(&source).expect("nothing here is doubly optional");

    // The model keeps what the manifest wrote: aliases aren't expanded.
    let scope = &manifest.components()["c"].scope;
    assert_eq!(scope["e"], Type::Named("Maybe".to_string()));
    assert_eq!(
        scope["d"],
        Type::Optional(Box::new(Type::Named("Chain".to_string())))
    );
    assert_eq!(
        manifest.expand(&scope["e"]),
        &Type::Optional(Box::new(Type::String))
    );
}

/// The invariant Pass 5's `is_assignable` relies on: in a loaded
/// manifest, no `optional` type wraps a type that expands to `optional`.
#[test]
fn no_loaded_optional_wraps_an_optional() {
    fn check(manifest: &mesh_manifest::Manifest, ty: &Type) {
        match ty {
            Type::Optional(inner) => {
                assert!(
                    !matches!(manifest.expand(inner), Type::Optional(_)),
                    "{ty:?} is doubly optional"
                );
                check(manifest, inner);
            }
            Type::List(element) => check(manifest, element),
            Type::Record(fields) => fields.values().for_each(|f| check(manifest, &f.ty)),
            _ => {}
        }
    }

    let example = example();
    let sources = [
        example.as_str(),
        r#"{ "version": 1, "types": {
            "Maybe": { "kind": "optional", "type": { "kind": "string" } },
            "R": { "kind": "record", "fields": { "m": { "type": { "kind": "named", "name": "Maybe" }, "required": true } } }
        }, "components": { "c": { "props": {}, "events": {}, "commands": {}, "scope": {
            "r": { "kind": "optional", "type": { "kind": "named", "name": "R" } }
        } } } }"#,
    ];
    for source in sources {
        let manifest = load(source).expect("should load");
        for ty in manifest.types().values() {
            check(&manifest, ty);
        }
        for component in manifest.components().values() {
            component
                .props
                .values()
                .for_each(|f| check(&manifest, &f.ty));
            component.scope.values().for_each(|ty| check(&manifest, ty));
            for event in component.events.values() {
                event.payload.iter().for_each(|ty| check(&manifest, ty));
            }
            for command in component.commands.values() {
                command
                    .parameters
                    .iter()
                    .for_each(|p| check(&manifest, &p.ty));
            }
        }
    }
}

/// A rejected value is reported once, and nothing inside it is checked.
#[test]
fn does_not_look_inside_a_rejected_value() {
    let void = r#"{ "kind": "void" }"#;
    let cases = [
        // An unknown property's value.
        (
            component(
                &format!(
                    r#"{{ "p": {{ "type": {{ "kind": "any" }}, "required": true, "default": {void} }} }}"#
                ),
                "{}",
                "{}",
                "{}",
            ),
            DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY,
        ),
        // A repeated key's second value.
        (
            component(
                &format!(
                    r#"{{ "p": {{ "type": {{ "kind": "any" }}, "required": true }}, "p": {{ "type": {void}, "required": true }} }}"#
                ),
                "{}",
                "{}",
                "{}",
            ),
            DiagnosticCode::MANIFEST_DUPLICATE_KEY,
        ),
        // A container of the wrong JSON type.
        (
            component(&format!("[{void}]"), "{}", "{}", "{}"),
            DiagnosticCode::MANIFEST_INVALID_VALUE,
        ),
        // A type without a "kind": its other properties mean nothing.
        (
            scope_type(&format!(r#"{{ "element": {void}, "bogus": 1 }}"#)),
            DiagnosticCode::MANIFEST_MISSING_PROPERTY,
        ),
        // A type with an unknown "kind", likewise.
        (
            scope_type(&format!(
                r#"{{ "kind": "tuple", "items": [{void}], "bogus": 1 }}"#
            )),
            DiagnosticCode::MANIFEST_UNKNOWN_KIND,
        ),
        // A "kind" that isn't a string, likewise.
        (
            scope_type(&format!(r#"{{ "kind": 1, "element": {void} }}"#)),
            DiagnosticCode::MANIFEST_INVALID_VALUE,
        ),
    ];
    for (source, code) in &cases {
        let codes: Vec<DiagnosticCode> = errors(source).into_iter().map(|(c, _)| c).collect();
        assert_eq!(codes, [*code], "{source}");
    }
}

/// An error in one value doesn't stop its siblings from being checked,
/// and a name error doesn't stop its declaration from being checked.
#[test]
fn keeps_checking_siblings_after_an_error() {
    assert_eq!(
        errors(&component(
            r#"{ "p": { "type": { "kind": "void" }, "required": "yes" } }"#,
            "[]",
            "{}",
            r#"{ "bad-name": { "kind": "list" } }"#
        )),
        [
            (DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""void""#),
            (DiagnosticCode::MANIFEST_INVALID_VALUE, r#""yes""#),
            (DiagnosticCode::MANIFEST_INVALID_VALUE, "[]"),
            (DiagnosticCode::MANIFEST_INVALID_NAME, r#""bad-name""#),
            (DiagnosticCode::MANIFEST_MISSING_PROPERTY, "{"),
        ]
    );
}

#[test]
fn a_schema_reference_must_be_a_string() {
    assert_eq!(
        errors(r#"{ "$schema": 1, "version": 1, "types": {}, "components": {} }"#),
        [(DiagnosticCode::MANIFEST_INVALID_VALUE, "1")]
    );
}

/// When a property is repeated, the first occurrence is the one that
/// counts, "kind" included.
#[test]
fn the_first_occurrence_of_a_repeated_property_counts() {
    assert_eq!(
        errors(&scope_type(
            r#"{ "kind": "list", "kind": "string", "element": { "kind": "any" } }"#
        )),
        [(DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#)]
    );
}

/// An invalid first "kind" stays authoritative: the valid second one is
/// reported as a repeat and not used, so the rest of the type object
/// isn't checked.
#[test]
fn a_valid_second_kind_does_not_replace_an_invalid_first_one() {
    let source = scope_type(r#"{ "kind": 123, "kind": "record", "fields": {} }"#);
    assert_eq!(
        errors(&source),
        [
            (DiagnosticCode::MANIFEST_INVALID_VALUE, "123"),
            (DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#),
        ]
    );
    // "fields" isn't checked: its bad name and bad kind go unreported.
    let source = scope_type(
        r#"{ "kind": 123, "kind": "record", "fields": { "bad-name": { "type": { "kind": "void" }, "required": true } } }"#,
    );
    assert_eq!(
        errors(&source),
        [
            (DiagnosticCode::MANIFEST_INVALID_VALUE, "123"),
            (DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#),
        ]
    );
    // Likewise after an unrecognised first "kind".
    assert_eq!(
        errors(&scope_type(
            r#"{ "kind": "tuple", "kind": "record", "fields": {} }"#
        )),
        [
            (DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""tuple""#),
            (DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#),
        ]
    );
}

/// A valid first "kind" stays authoritative: an invalid second one is
/// reported as a repeat and not used, and the rest of the type object is
/// checked by the first kind's rules.
#[test]
fn an_invalid_second_kind_does_not_replace_a_valid_first_one() {
    let source = scope_type(r#"{ "kind": "record", "kind": "garbage", "fields": {} }"#);
    assert_eq!(
        errors(&source),
        [(DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#)]
    );
    // "fields" is checked as a record's fields.
    let source = scope_type(
        r#"{ "kind": "record", "kind": "garbage", "fields": { "bad-name": { "type": { "kind": "void" }, "required": true } } }"#,
    );
    assert_eq!(
        errors(&source),
        [
            (DiagnosticCode::MANIFEST_DUPLICATE_KEY, r#""kind""#),
            (DiagnosticCode::MANIFEST_INVALID_NAME, r#""bad-name""#),
            (DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""void""#),
        ]
    );
}

/// A reference to a declared type whose definition has errors is not
/// also unknown, or anything else: the definition's error is enough.
#[test]
fn a_declared_type_with_errors_is_not_unknown() {
    let source = manifest(
        r#"{ "T": { "kind": "void" } }"#,
        r#"{ "c": { "props": {}, "events": {}, "commands": {}, "scope": {
            "x": { "kind": "named", "name": "T" },
            "y": { "kind": "optional", "type": { "kind": "named", "name": "T" } }
        } } }"#,
    );
    assert_eq!(
        errors(&source),
        [(DiagnosticCode::MANIFEST_UNKNOWN_KIND, r#""void""#)]
    );
}

/// Diagnostics are sorted by where they start. Two that start at the same
/// byte keep the order the phases emit them in.
#[test]
fn orders_diagnostics_at_the_same_position_deterministically() {
    // Missing properties are all reported at the object's `{`, in the
    // order the format lists them.
    let source = component(r#"{ "p": {} }"#, "{}", "{}", "{}");
    let diagnostics = load(&source).expect_err("an empty field declaration");
    let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            r#"prop "p" is missing the property "type""#,
            r#"prop "p" is missing the property "required""#,
        ]
    );

    // A name error (a declaration check) comes before a recursion error
    // (a graph check) on the same key.
    let source = manifest(
        r#"{ "Bad-T": { "kind": "list", "element": { "kind": "named", "name": "Bad-T" } } }"#,
        "{}",
    );
    assert_eq!(
        errors(&source),
        [
            (DiagnosticCode::MANIFEST_INVALID_NAME, r#""Bad-T""#),
            (DiagnosticCode::MANIFEST_RECURSIVE_TYPE, r#""Bad-T""#),
        ]
    );
}

/// Each diagnostic covers exactly the offending key or value: raw byte
/// offsets, no surrounding whitespace, escapes as written, after a BOM and
/// multi-byte text.
#[test]
fn diagnostic_spans_cover_exactly_the_offending_token() {
    let source = "\u{feff}{ \"version\" : 1 ,\n\t\"types\"\t:\t{\n    \"名前\" :\r\n {\"kind\":\"any\"}\n  },\n  \"components\": { \"c\" : {\n    \"props\" : { \"aria\\u002dlabel\"  :  { \"type\" : { \"kind\" : \"string\" } ,  \"required\"  :   \"no\"   } } ,\n    \"events\": {}, \"commands\": {},\n    \"scope\": { \"u\" : { \"kind\": \"named\" ,\"name\"  :\"Usr\" } , \"v\": { \"kind\" : \"any\" ,\t\"extra\" : 1 } }\n  } }\n}";
    assert_eq!(
        errors(source),
        [
            (DiagnosticCode::MANIFEST_INVALID_NAME, "\"名前\""),
            (
                DiagnosticCode::MANIFEST_INVALID_NAME,
                "\"aria\\u002dlabel\""
            ),
            (DiagnosticCode::MANIFEST_INVALID_VALUE, "\"no\""),
            (DiagnosticCode::MANIFEST_UNKNOWN_TYPE, "\"Usr\""),
            (DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY, "\"extra\""),
        ]
    );
}
