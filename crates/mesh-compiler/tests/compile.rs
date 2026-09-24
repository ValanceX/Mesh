use mesh_semantic::{AttributeValue, Child, Element, Expression};
use mesh_syntax::Span;

/// The span every span becomes in [`without_spans`].
const NO_SPAN: Span = Span {
    start_byte: 0,
    end_byte: 0,
};

/// `element` with every span replaced by [`NO_SPAN`], so a test about
/// the IR's shape can compare it with an expected value written without
/// real offsets. `every_construct_has_its_own_span` pins the spans
/// themselves.
fn without_spans(mut element: Element) -> Element {
    clear_element(&mut element);
    element
}

fn clear_element(element: &mut Element) {
    element.name_span = NO_SPAN;
    element.span = NO_SPAN;
    for attribute in &mut element.attributes {
        attribute.name_span = NO_SPAN;
        attribute.span = NO_SPAN;
        match &mut attribute.value {
            AttributeValue::String { span, .. } => *span = NO_SPAN,
            AttributeValue::Expression(expression) => clear_expression(expression),
        }
    }
    for binding in &mut element.event_bindings {
        binding.name_span = NO_SPAN;
        binding.span = NO_SPAN;
        clear_expression(&mut binding.handler);
    }
    for child in &mut element.children {
        match child {
            Child::Text { span, .. } => *span = NO_SPAN,
            Child::Expression(expression) => clear_expression(expression),
            Child::Element(element) => clear_element(element),
        }
    }
}

fn clear_expression(expression: &mut Expression) {
    match expression {
        Expression::Literal { span, .. }
        | Expression::Reference { span, .. }
        | Expression::EventValue { span, .. } => *span = NO_SPAN,
        Expression::MemberAccess {
            object,
            property_span,
            span,
            ..
        } => {
            clear_expression(object);
            *property_span = NO_SPAN;
            *span = NO_SPAN;
        }
        Expression::Unary { operand, span, .. } => {
            clear_expression(operand);
            *span = NO_SPAN;
        }
        Expression::Binary {
            left, right, span, ..
        } => {
            clear_expression(left);
            clear_expression(right);
            *span = NO_SPAN;
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            span,
        } => {
            clear_expression(condition);
            clear_expression(consequent);
            clear_expression(alternate);
            *span = NO_SPAN;
        }
        Expression::Array { elements, span } => {
            elements.iter_mut().for_each(clear_expression);
            *span = NO_SPAN;
        }
        Expression::Object { members, span } => {
            for member in members {
                member.key_span = NO_SPAN;
                member.span = NO_SPAN;
                clear_expression(&mut member.value);
            }
            *span = NO_SPAN;
        }
        Expression::Command {
            command_span,
            arguments,
            span,
            ..
        } => {
            *command_span = NO_SPAN;
            arguments.iter_mut().for_each(clear_expression);
            *span = NO_SPAN;
        }
    }
}

#[test]
fn compiles_valid_source_with_no_diagnostics() {
    let result = mesh_compiler::compile(r#"<page title="Users" />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(ir.name, "page");
    assert_eq!(ir.attributes[0].name, "title");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::String {
            value: "Users".to_string(),
            span: NO_SPAN
        }
    );
}

#[test]
fn compiles_container_element_and_preserves_text_content() {
    let result = mesh_compiler::compile("<title>Users</title>");

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(ir.name, "title");
    assert_eq!(ir.children.len(), 1);
    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Text {
            value: "Users".to_string(),
            span: NO_SPAN
        }
    );
}

#[test]
fn compiles_nested_elements_with_surrounding_whitespace() {
    let result = mesh_compiler::compile("<div>\n  <span>A</span>\n  <span>B</span>\n</div>\n");

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
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
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::MemberAccess {
            object: Box::new(mesh_semantic::Expression::Reference {
                name: "user".to_string(),
                span: NO_SPAN
            }),
            property: "name".to_string(),
            property_span: NO_SPAN,
            span: NO_SPAN,
        })
    );
}

#[test]
fn compiles_a_child_content_expression() {
    let result = mesh_compiler::compile("<title>{user}</title>");

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Expression(mesh_semantic::Expression::Reference {
            name: "user".to_string(),
            span: NO_SPAN
        })
    );
}

#[test]
fn compiles_boolean_and_null_literal_attributes() {
    let result = mesh_compiler::compile(r#"<page disabled={true} value={null} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal {
            value: mesh_semantic::Literal::Boolean(true),
            span: NO_SPAN
        })
    );
    assert_eq!(
        ir.attributes[1].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal {
            value: mesh_semantic::Literal::Null,
            span: NO_SPAN
        })
    );
}

#[test]
fn compiles_a_string_literal_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page value={"hi"} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Literal {
            value: mesh_semantic::Literal::String("hi".to_string()),
            span: NO_SPAN
        })
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

    assert_eq!(
        diagnostic.to_string(),
        "unterminated tag `<page`: expected `>` or `/>` (0..5)"
    );
}

#[test]
fn compiles_a_unary_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page disabled={!enabled} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Unary {
            operator: mesh_syntax::UnaryOperator::Not,
            operand: Box::new(mesh_semantic::Expression::Reference {
                name: "enabled".to_string(),
                span: NO_SPAN
            }),
            span: NO_SPAN,
        })
    );
}

#[test]
fn compiles_a_binary_expression_attribute_respecting_precedence() {
    let result = mesh_compiler::compile(r#"<page total={a + b * c} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Binary {
            operator: mesh_syntax::BinaryOperator::Add,
            left: Box::new(mesh_semantic::Expression::Reference {
                name: "a".to_string(),
                span: NO_SPAN
            }),
            right: Box::new(mesh_semantic::Expression::Binary {
                operator: mesh_syntax::BinaryOperator::Mul,
                left: Box::new(mesh_semantic::Expression::Reference {
                    name: "b".to_string(),
                    span: NO_SPAN
                }),
                right: Box::new(mesh_semantic::Expression::Reference {
                    name: "c".to_string(),
                    span: NO_SPAN
                }),
                span: NO_SPAN,
            }),
            span: NO_SPAN,
        })
    );
}

#[test]
fn compiles_a_conditional_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page size={compact ? "sm" : "md"} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Conditional {
            condition: Box::new(mesh_semantic::Expression::Reference {
                name: "compact".to_string(),
                span: NO_SPAN
            }),
            consequent: Box::new(mesh_semantic::Expression::Literal {
                value: mesh_semantic::Literal::String("sm".to_string()),
                span: NO_SPAN
            }),
            alternate: Box::new(mesh_semantic::Expression::Literal {
                value: mesh_semantic::Literal::String("md".to_string()),
                span: NO_SPAN
            }),
            span: NO_SPAN,
        })
    );
}

#[test]
fn compiles_an_array_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page items={[1, 2, 3]} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Array {
            elements: vec![
                mesh_semantic::Expression::Literal {
                    value: mesh_semantic::Literal::Number("1".to_string()),
                    span: NO_SPAN
                },
                mesh_semantic::Expression::Literal {
                    value: mesh_semantic::Literal::Number("2".to_string()),
                    span: NO_SPAN
                },
                mesh_semantic::Expression::Literal {
                    value: mesh_semantic::Literal::Number("3".to_string()),
                    span: NO_SPAN
                },
            ],
            span: NO_SPAN
        })
    );
}

#[test]
fn compiles_an_object_expression_attribute() {
    let result = mesh_compiler::compile(r#"<page data={{ name: "Users" }} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Object {
            members: vec![mesh_semantic::ObjectMember {
                key: "name".to_string(),
                value: mesh_semantic::Expression::Literal {
                    value: mesh_semantic::Literal::String("Users".to_string()),
                    span: NO_SPAN
                },
                key_span: NO_SPAN,
                span: NO_SPAN,
            },],
            span: NO_SPAN
        })
    );
}

#[test]
fn compiles_a_command_invocation_attribute() {
    let result = mesh_compiler::compile(r#"<page action={selectUser($event)} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Command {
            command: "selectUser".to_string(),
            arguments: vec![mesh_semantic::Expression::EventValue {
                name: "event".to_string(),
                span: NO_SPAN
            }],
            command_span: NO_SPAN,
            span: NO_SPAN,
        })
    );
}

#[test]
fn compiles_an_event_value_attribute() {
    let result = mesh_compiler::compile(r#"<page handler={$event} />"#);

    assert!(result.diagnostics.is_empty());
    let ir = without_spans(result.ir.expect("should produce IR"));
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::EventValue {
            name: "event".to_string(),
            span: NO_SPAN
        })
    );
}

#[test]
fn compiling_an_event_binding_produces_it_in_the_ir_with_no_diagnostics() {
    let result = mesh_compiler::compile(r#"<div on.click={handler} />"#);

    let element = without_spans(result.ir.expect("should compile"));
    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    match &element.event_bindings[0].handler {
        mesh_semantic::Expression::Reference { name, .. } => assert_eq!(name, "handler"),
        other => panic!("expected a reference expression handler, got {other:?}"),
    }

    assert!(result.diagnostics.is_empty());
}

#[test]
fn compiling_duplicate_attributes_produces_warning_diagnostics_and_still_compiles() {
    let result = mesh_compiler::compile(r#"<div class="a" class="b" />"#);

    let element = without_spans(result.ir.expect("should compile"));
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

    let element = without_spans(result.ir.expect("should compile"));
    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    match &element.event_bindings[0].handler {
        mesh_semantic::Expression::Reference { name, .. } => assert_eq!(name, "b"),
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

#[test]
fn every_diagnostic_carries_its_code() {
    use mesh_syntax::DiagnosticCode;

    let result =
        mesh_compiler::compile(r#"<div class="a" class="b" on.click={x} on.click={y}></span>"#);
    let codes: Vec<DiagnosticCode> = result.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [
            DiagnosticCode::DUPLICATE_ATTRIBUTE,
            DiagnosticCode::DUPLICATE_EVENT_BINDING,
            DiagnosticCode::MISMATCHED_CLOSING_TAG,
        ]
    );

    let result = mesh_compiler::compile("<page");
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::UNTERMINATED_TAG);
}

/// The source text a span covers.
fn slice(source: &str, span: Span) -> &str {
    &source[span.start_byte..span.end_byte]
}

/// Every construct that can be wrong on its own (D11) has its own span in
/// the IR: tag names, attribute names and values, event names and
/// handlers, command names and arguments, both halves of a member access,
/// object keys and values, array elements, and text.
#[test]
fn every_construct_has_its_own_span() {
    let source = concat!(
        "<user-card\n",
        "  user={user}\n",
        "  compact={layout.compact}\n",
        "  data={{ k: [1, -n], \"q-r\": a ? b : c }}\n",
        "  label=\"Hi\"\n",
        "  on.select={selectUser($event, x == y)}>\n",
        "  Hello {name}\n",
        "</user-card>\n",
    );
    let result = mesh_compiler::compile(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let ir = result.ir.expect("should produce IR");

    assert_eq!(slice(source, ir.name_span), "user-card");
    assert!(slice(source, ir.span).starts_with("<user-card\n"));
    assert!(slice(source, ir.span).ends_with("</user-card>"));

    let [user, compact, data, label] = &ir.attributes[..] else {
        panic!("expected four attributes, got {:?}", ir.attributes);
    };
    assert_eq!(slice(source, user.name_span), "user");
    assert_eq!(slice(source, user.span), "user={user}");
    assert_eq!(slice(source, user.value.span()), "user");

    let AttributeValue::Expression(Expression::MemberAccess {
        object,
        property_span,
        span,
        ..
    }) = &compact.value
    else {
        panic!("expected a member access, got {:?}", compact.value);
    };
    assert_eq!(slice(source, object.span()), "layout");
    assert_eq!(slice(source, *property_span), "compact");
    assert_eq!(slice(source, *span), "layout.compact");

    let AttributeValue::Expression(Expression::Object { members, span }) = &data.value else {
        panic!("expected an object, got {:?}", data.value);
    };
    assert_eq!(slice(source, *span), "{ k: [1, -n], \"q-r\": a ? b : c }");
    assert_eq!(slice(source, members[0].key_span), "k");
    assert_eq!(slice(source, members[0].span), "k: [1, -n]");
    let Expression::Array { elements, span } = &members[0].value else {
        panic!("expected an array, got {:?}", members[0].value);
    };
    assert_eq!(slice(source, *span), "[1, -n]");
    assert_eq!(slice(source, elements[0].span()), "1");
    assert_eq!(slice(source, elements[1].span()), "-n");
    assert_eq!(slice(source, members[1].key_span), "\"q-r\"");
    assert_eq!(slice(source, members[1].value.span()), "a ? b : c");

    assert_eq!(slice(source, label.value.span()), "\"Hi\"");

    let binding = &ir.event_bindings[0];
    assert_eq!(slice(source, binding.name_span), "select");
    assert_eq!(
        slice(source, binding.span),
        "on.select={selectUser($event, x == y)}"
    );
    let Expression::Command {
        command_span,
        arguments,
        span,
        ..
    } = &binding.handler
    else {
        panic!("expected a command, got {:?}", binding.handler);
    };
    assert_eq!(slice(source, *command_span), "selectUser");
    assert_eq!(slice(source, *span), "selectUser($event, x == y)");
    assert_eq!(slice(source, arguments[0].span()), "$event");
    assert_eq!(slice(source, arguments[1].span()), "x == y");

    let [Child::Text { value, span }, Child::Expression(name)] = &ir.children[..] else {
        panic!("expected text then an expression, got {:?}", ir.children);
    };
    assert_eq!(slice(source, *span), value.as_str());
    assert_eq!(value, "\n  Hello ");
    assert_eq!(slice(source, name.span()), "name");
}

#[test]
fn ir_spans_are_raw_byte_offsets() {
    // A BOM and a multi-byte character come before the spans: they are
    // not shifted or counted in characters.
    let source = "\u{feff}<page title=\"é\" user={a.b} />";
    let ir = mesh_compiler::compile(source)
        .ir
        .expect("should produce IR");

    assert_eq!(ir.name_span.start_byte, 4);
    assert_eq!(slice(source, ir.name_span), "page");
    assert_eq!(slice(source, ir.attributes[0].value.span()), "\"é\"");
    assert_eq!(slice(source, ir.attributes[1].value.span()), "a.b");
}

#[test]
fn a_parenthesized_expression_span_excludes_the_parentheses() {
    let source = "<page n={(a + b)} />";
    let ir = mesh_compiler::compile(source)
        .ir
        .expect("should produce IR");

    assert_eq!(slice(source, ir.attributes[0].value.span()), "a + b");
}

/// Compiles `source`, which must be valid, and returns its IR.
fn ir_of(source: &str) -> Element {
    let result = mesh_compiler::compile(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.ir.expect("should produce IR")
}

/// The expression value of the attribute at `index`.
fn attribute_expression(ir: &Element, index: usize) -> &Expression {
    match &ir.attributes[index].value {
        AttributeValue::Expression(expression) => expression,
        other => panic!("expected an expression, got {other:?}"),
    }
}

// One test per D11 construct, so a wrong span in one can't be hidden by
// another. Each source starts with a BOM and has multi-byte text before
// the construct, and inside it where MPRX allows non-ASCII (names are
// ASCII-only, so only values can contain it).

#[test]
fn d11_tag_name() {
    let source = "\u{feff}<page title=\"日本\">\n  <my-card label=\"é\" />\n</page>";
    let ir = ir_of(source);
    assert_eq!(slice(source, ir.name_span), "page");
    let Child::Element(card) = &ir.children[0] else {
        panic!("expected an element, got {:?}", ir.children[0]);
    };
    assert_eq!(slice(source, card.name_span), "my-card");
}

#[test]
fn d11_attribute_name_and_value() {
    let source = "\u{feff}<page data=\"héllo\" title={\"日本\"} />";
    let ir = ir_of(source);
    assert_eq!(slice(source, ir.attributes[0].name_span), "data");
    assert_eq!(slice(source, ir.attributes[0].value.span()), "\"héllo\"");
    assert_eq!(slice(source, ir.attributes[1].name_span), "title");
    assert_eq!(slice(source, ir.attributes[1].value.span()), "\"日本\"");
}

#[test]
fn d11_event_name_and_handler() {
    let source = "\u{feff}<button label=\"é\" on.click={go(\"日本\")} />";
    let ir = ir_of(source);
    let binding = &ir.event_bindings[0];
    assert_eq!(slice(source, binding.name_span), "click");
    assert_eq!(slice(source, binding.handler.span()), "go(\"日本\")");
}

#[test]
fn d11_command_name_and_each_argument() {
    let source = "\u{feff}<button label=\"é\" on.click={selectUser(\"日本\", a.b, $event)} />";
    let ir = ir_of(source);
    let Expression::Command {
        command_span,
        arguments,
        ..
    } = &ir.event_bindings[0].handler
    else {
        panic!("expected a command, got {:?}", ir.event_bindings[0].handler);
    };
    assert_eq!(slice(source, *command_span), "selectUser");
    let arguments: Vec<&str> = arguments.iter().map(|a| slice(source, a.span())).collect();
    assert_eq!(arguments, ["\"日本\"", "a.b", "$event"]);
}

#[test]
fn d11_member_access_object_property_and_whole() {
    let source = "\u{feff}<page label=\"é\" compact={layout.options.compact} />";
    let ir = ir_of(source);
    let Expression::MemberAccess {
        object,
        property_span,
        span,
        ..
    } = attribute_expression(&ir, 1)
    else {
        panic!("expected a member access");
    };
    assert_eq!(slice(source, *span), "layout.options.compact");
    assert_eq!(slice(source, *property_span), "compact");
    assert_eq!(slice(source, object.span()), "layout.options");
    let Expression::MemberAccess {
        object: inner,
        property_span: inner_property,
        ..
    } = object.as_ref()
    else {
        panic!("expected a nested member access");
    };
    assert_eq!(slice(source, *inner_property), "options");
    assert_eq!(slice(source, inner.span()), "layout");
}

#[test]
fn d11_object_keys_and_values() {
    let source =
        "\u{feff}<page label=\"é\" data={{ plain: 1, \"日-本\": \"ü\", \"a\\\"b\": [x] }} />";
    let ir = ir_of(source);
    let Expression::Object { members, .. } = attribute_expression(&ir, 1) else {
        panic!("expected an object");
    };
    let keys: Vec<&str> = members.iter().map(|m| slice(source, m.key_span)).collect();
    assert_eq!(keys, ["plain", "\"日-本\"", "\"a\\\"b\""]);
    let values: Vec<&str> = members
        .iter()
        .map(|m| slice(source, m.value.span()))
        .collect();
    assert_eq!(values, ["1", "\"ü\"", "[x]"]);
    assert_eq!(members[2].key, "a\"b");
}

#[test]
fn d11_array_elements() {
    let source = "\u{feff}<page label=\"é\" items={[ \"日本\" , -n, a ? b : c, [] ]} />";
    let ir = ir_of(source);
    let Expression::Array { elements, .. } = attribute_expression(&ir, 1) else {
        panic!("expected an array");
    };
    let elements: Vec<&str> = elements.iter().map(|e| slice(source, e.span())).collect();
    assert_eq!(elements, ["\"日本\"", "-n", "a ? b : c", "[]"]);
}

#[test]
fn d11_child_text_and_expression() {
    let source = "\u{feff}<text>héllo {user.name} 日本</text>";
    let ir = ir_of(source);
    let children: Vec<&str> = ir
        .children
        .iter()
        .map(|child| match child {
            Child::Text { span, .. } => slice(source, *span),
            Child::Expression(expression) => slice(source, expression.span()),
            Child::Element(element) => slice(source, element.span),
        })
        .collect();
    assert_eq!(children, ["héllo ", "user.name", " 日本"]);
}
