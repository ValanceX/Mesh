use mesh_syntax::Span;

#[test]
fn parses_a_self_closing_element_with_a_string_attribute() {
    let element = mesh_parser::parse(r#"<page title="Users" />"#).expect("should parse");

    assert_eq!(element.name, "page");
    assert_eq!(element.attributes.len(), 1);
    assert_eq!(element.attributes[0].name, "title");
    assert_eq!(element.attributes[0].value.value, "Users");
    assert!(element.children.is_empty());
}

#[test]
fn parses_a_container_element_with_text() {
    let element = mesh_parser::parse("<title>Users</title>").expect("should parse");

    assert_eq!(element.name, "title");
    assert_eq!(element.children.len(), 1);
    assert_eq!(element.children[0].value, "Users");
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
