use mesh_syntax::Span;

#[test]
fn parses_a_self_closing_element_with_a_string_attribute() {
    let element = mesh_parser::parse(r#"<page title="Users" />"#).expect("should parse");

    assert_eq!(element.name, "page");
    assert_eq!(element.attributes.len(), 1);
    assert_eq!(element.attributes[0].name, "title");
    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::String(literal) => assert_eq!(literal.value, "Users"),
        other => panic!("expected a string attribute value, got {other:?}"),
    }
    assert!(element.children.is_empty());
}

#[test]
fn parses_a_container_element_with_text() {
    let element = mesh_parser::parse("<title>Users</title>").expect("should parse");

    assert_eq!(element.name, "title");
    assert_eq!(element.children.len(), 1);
    match &element.children[0] {
        mesh_syntax::Child::Text(text) => assert_eq!(text.value, "Users"),
        other => panic!("expected a text child, got {other:?}"),
    }
}

#[test]
fn parses_a_container_element_with_an_expression_child() {
    let element = mesh_parser::parse("<title>{user}</title>").expect("should parse");

    assert_eq!(element.children.len(), 1);
    match &element.children[0] {
        mesh_syntax::Child::Expression(mesh_syntax::Expression::Reference(reference)) => {
            assert_eq!(reference.name, "user");
        }
        other => panic!("expected a reference expression child, got {other:?}"),
    }
}

#[test]
fn reports_a_parse_error_for_invalid_source() {
    let error = mesh_parser::parse("<page").expect_err("should fail to parse");
    assert!(!error.message.is_empty());
    let _: Span = error.span;
}

#[test]
fn parses_a_root_element_preceded_by_whitespace() {
    let element = mesh_parser::parse("\n<page title=\"Users\" />")
        .expect("leading whitespace before the root element should still parse");

    assert_eq!(element.name, "page");
}

#[test]
fn parses_a_reference_expression_attribute() {
    let element = mesh_parser::parse(r#"<page title={title} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Reference(reference)) => {
            assert_eq!(reference.name, "title");
        }
        other => panic!("expected a reference expression, got {other:?}"),
    }
}

#[test]
fn parses_a_member_access_expression_attribute() {
    let element = mesh_parser::parse(r#"<page title={user.name} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::MemberAccess(member)) => {
            assert_eq!(member.property, "name");
            match member.object.as_ref() {
                mesh_syntax::Expression::Reference(reference) => assert_eq!(reference.name, "user"),
                other => panic!("expected object to be a reference, got {other:?}"),
            }
        }
        other => panic!("expected a member access expression, got {other:?}"),
    }
}

#[test]
fn parses_a_chained_member_access_expression_attribute() {
    let element = mesh_parser::parse(r#"<page value={a.b.c} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::MemberAccess(outer)) => {
            assert_eq!(outer.property, "c");
            match outer.object.as_ref() {
                mesh_syntax::Expression::MemberAccess(inner) => {
                    assert_eq!(inner.property, "b");
                    match inner.object.as_ref() {
                        mesh_syntax::Expression::Reference(reference) => {
                            assert_eq!(reference.name, "a");
                        }
                        other => {
                            panic!("expected innermost object to be a reference, got {other:?}")
                        }
                    }
                }
                other => panic!("expected outer object to be a member access, got {other:?}"),
            }
        }
        other => panic!("expected a member access expression, got {other:?}"),
    }
}

#[test]
fn parses_a_number_literal_expression_attribute() {
    let element = mesh_parser::parse(r#"<page count={5} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
            mesh_syntax::Literal::Number(number),
        )) => {
            assert_eq!(number.value, "5");
        }
        other => panic!("expected a number literal expression, got {other:?}"),
    }
}

#[test]
fn parses_a_boolean_literal_expression_attribute() {
    let element = mesh_parser::parse(r#"<page disabled={true} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
            mesh_syntax::Literal::Boolean(boolean),
        )) => {
            assert!(boolean.value);
        }
        other => panic!("expected a boolean literal expression, got {other:?}"),
    }
}

#[test]
fn parses_a_null_literal_expression_attribute() {
    let element = mesh_parser::parse(r#"<page value={null} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
            mesh_syntax::Literal::Null(_),
        )) => {}
        other => panic!("expected a null literal expression, got {other:?}"),
    }
}

#[test]
fn parses_multiple_mixed_children() {
    let element = mesh_parser::parse("<title>Hello {user}!</title>").expect("should parse");

    assert_eq!(element.children.len(), 3);
    match &element.children[0] {
        mesh_syntax::Child::Text(text) => assert_eq!(text.value, "Hello "),
        other => panic!("expected first child to be text, got {other:?}"),
    }
    match &element.children[1] {
        mesh_syntax::Child::Expression(mesh_syntax::Expression::Reference(reference)) => {
            assert_eq!(reference.name, "user");
        }
        other => panic!("expected second child to be a reference expression, got {other:?}"),
    }
    match &element.children[2] {
        mesh_syntax::Child::Text(text) => assert_eq!(text.value, "!"),
        other => panic!("expected third child to be text, got {other:?}"),
    }
}

#[test]
fn parses_a_string_literal_expression_attribute() {
    let element = mesh_parser::parse(r#"<page value={"hi"} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
            mesh_syntax::Literal::String(literal),
        )) => {
            assert_eq!(literal.value, "hi");
        }
        other => panic!("expected a string literal expression, got {other:?}"),
    }
}

#[test]
fn parses_a_negative_decimal_number_literal_expression_attribute() {
    let element = mesh_parser::parse(r#"<page count={-3.5} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
            mesh_syntax::Literal::Number(number),
        )) => {
            assert_eq!(number.value, "-3.5");
        }
        other => panic!("expected a number literal expression, got {other:?}"),
    }
}

#[test]
fn preserves_whitespace_only_text_children() {
    // Pins current behavior: MPRX does not trim whitespace-only text
    // children (unlike JSX). This is an open question for Pass 4 to
    // revisit deliberately, not a decision this test is making — see
    // docs/MPRX-SPEC.md §3.
    let element = mesh_parser::parse("<title>\n  {user}\n</title>").expect("should parse");

    assert_eq!(element.children.len(), 3);
    match &element.children[0] {
        mesh_syntax::Child::Text(text) => assert_eq!(text.value, "\n  "),
        other => panic!("expected first child to be whitespace text, got {other:?}"),
    }
    match &element.children[2] {
        mesh_syntax::Child::Text(text) => assert_eq!(text.value, "\n"),
        other => panic!("expected third child to be whitespace text, got {other:?}"),
    }
}

#[test]
fn decodes_string_escape_sequences() {
    let element = mesh_parser::parse(r#"<page title="She said \"hi\"" />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::String(literal) => {
            assert_eq!(literal.value, "She said \"hi\"");
        }
        other => panic!("expected a string attribute value, got {other:?}"),
    }
}
