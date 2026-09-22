#[test]
fn compiles_valid_source_with_no_diagnostics() {
    let result = mesh_compiler::compile(r#"<page title="Users" />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(ir.name, "page");
    assert_eq!(ir.attributes[0].name, "title");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::String("Users".to_string())
    );
}

#[test]
fn compiles_container_element_and_preserves_text_content() {
    let result = mesh_compiler::compile("<title>Users</title>");

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(ir.name, "title");
    assert_eq!(ir.children.len(), 1);
    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Text("Users".to_string())
    );
}

#[test]
fn compiles_a_member_access_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page title={user.name} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::MemberAccess {
            object: Box::new(mesh_semantic::Expression::Reference("user".to_string())),
            property: "name".to_string(),
        })
    );
}

#[test]
fn compiles_a_child_content_expression() {
    let result = mesh_compiler::compile("<title>{user}</title>");

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Expression(mesh_semantic::Expression::Reference("user".to_string()))
    );
}

#[test]
fn compiles_boolean_and_null_literal_attributes() {
    let result = mesh_compiler::compile(r#"<page disabled={true} value={null} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal(
            mesh_semantic::Literal::Boolean(true)
        ))
    );
    assert_eq!(
        ir.attributes[1].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal(
            mesh_semantic::Literal::Null
        ))
    );
}

#[test]
fn compiles_a_string_literal_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page value={"hi"} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal(
            mesh_semantic::Literal::String("hi".to_string())
        ))
    );
}

#[test]
fn compiling_invalid_source_produces_an_error_diagnostic() {
    let result = mesh_compiler::compile("<page");

    assert!(result.ir.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].severity,
        mesh_compiler::Severity::Error
    );
}

#[test]
fn diagnostic_display_includes_message_and_span() {
    let result = mesh_compiler::compile("<page");
    let diagnostic = &result.diagnostics[0];

    assert_eq!(diagnostic.to_string(), "syntax error (0..5)");
}

#[test]
fn compiles_a_unary_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page disabled={!enabled} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Unary {
            operator: mesh_syntax::UnaryOperator::Not,
            operand: Box::new(mesh_semantic::Expression::Reference("enabled".to_string())),
        })
    );
}

#[test]
fn compiles_a_binary_expression_attribute_respecting_precedence() {
    let result = mesh_compiler::compile(r#"<page total={a + b * c} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Binary {
            operator: mesh_syntax::BinaryOperator::Add,
            left: Box::new(mesh_semantic::Expression::Reference("a".to_string())),
            right: Box::new(mesh_semantic::Expression::Binary {
                operator: mesh_syntax::BinaryOperator::Mul,
                left: Box::new(mesh_semantic::Expression::Reference("b".to_string())),
                right: Box::new(mesh_semantic::Expression::Reference("c".to_string())),
            }),
        })
    );
}

#[test]
fn compiles_a_conditional_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page size={compact ? "sm" : "md"} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Conditional {
            condition: Box::new(mesh_semantic::Expression::Reference("compact".to_string())),
            consequent: Box::new(mesh_semantic::Expression::Literal(
                mesh_semantic::Literal::String("sm".to_string())
            )),
            alternate: Box::new(mesh_semantic::Expression::Literal(
                mesh_semantic::Literal::String("md".to_string())
            )),
        })
    );
}
