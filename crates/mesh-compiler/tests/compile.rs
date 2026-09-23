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
fn compiles_nested_elements_with_surrounding_whitespace() {
    let result = mesh_compiler::compile("<div>\n  <span>A</span>\n  <span>B</span>\n</div>\n");

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(ir.name, "div");

    // Whitespace-only text children are filtered at the IR level (Task 5),
    // so only the two `span` elements should remain.
    assert_eq!(ir.children.len(), 2);
    for child in &ir.children {
        match child {
            mesh_semantic::Child::Element(inner) => assert_eq!(inner.name, "span"),
            other => panic!("expected an element child, got {other:?}"),
        }
    }
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
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Error);
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

#[test]
fn compiles_an_array_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page items={[1, 2, 3]} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Array(vec![
            mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number("1".to_string())),
            mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number("2".to_string())),
            mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number("3".to_string())),
        ]))
    );
}

#[test]
fn compiles_an_object_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page data={{ name: "Users" }} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Object(vec![
            mesh_semantic::ObjectMember {
                key: "name".to_string(),
                value: mesh_semantic::Expression::Literal(mesh_semantic::Literal::String(
                    "Users".to_string()
                )),
            },
        ]))
    );
}

#[test]
fn compiles_a_command_invocation_attribute() {
    let result = mesh_compiler::compile(r#"<page action={selectUser($event)} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Command {
            command: "selectUser".to_string(),
            arguments: vec![mesh_semantic::Expression::EventValue("event".to_string())],
        })
    );
}

#[test]
fn compiles_an_event_value_attribute() {
    let result = mesh_compiler::compile(r#"<page handler={$event} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = result.ir.expect("should produce IR");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::EventValue(
            "event".to_string()
        ))
    );
}

#[test]
fn compiling_an_event_binding_produces_it_in_the_ir_with_no_diagnostics() {
    let result = mesh_compiler::compile(r#"<div on.click={handler} />"#);

    let element = result.ir.expect("should compile");
    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    match &element.event_bindings[0].handler {
        mesh_semantic::Expression::Reference(reference) => assert_eq!(reference, "handler"),
        other => panic!("expected a reference expression handler, got {other:?}"),
    }

    assert!(result.diagnostics.is_empty());
}

#[test]
fn compiling_duplicate_attributes_produces_warning_diagnostics_and_still_compiles() {
    let result = mesh_compiler::compile(r#"<div class="a" class="b" />"#);

    let element = result.ir.expect("should compile");
    assert_eq!(element.attributes.len(), 1);
    assert_eq!(element.attributes[0].name, "class");

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].severity,
        mesh_syntax::Severity::Warning
    );
}

#[test]
fn compiling_duplicate_event_bindings_produces_warning_diagnostics_and_still_compiles() {
    let source = r#"<div on.click={a} on.click={b} />"#;
    let result = mesh_compiler::compile(source);

    let element = result.ir.expect("should compile");
    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    match &element.event_bindings[0].handler {
        mesh_semantic::Expression::Reference(reference) => assert_eq!(reference, "b"),
        other => panic!("expected a reference expression handler, got {other:?}"),
    }

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].severity,
        mesh_syntax::Severity::Warning
    );
    assert_eq!(
        result.diagnostics[0].message,
        "duplicate event binding \"click\": this occurrence is shadowed by a later one"
    );
    // The diagnostic's span must point at the shadowed *earlier*
    // occurrence (`on.click={a}`), never the surviving one
    // (`on.click={b}`). Slicing the original source with the returned
    // span — rather than hardcoding byte offsets — makes this assertion
    // self-verifying against whatever span convention the parser
    // actually uses.
    let diagnostic_span = result.diagnostics[0].span;
    assert_eq!(
        &source[diagnostic_span.start_byte..diagnostic_span.end_byte],
        "on.click={a}"
    );
}

#[test]
fn compiling_a_mismatched_closing_tag_produces_a_non_fatal_error_diagnostic() {
    let result = mesh_compiler::compile("<div>hi</span>");

    let element = result
        .ir
        .expect("should still compile despite the mismatch");
    assert_eq!(element.name, "div");

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Error);
}
