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
fn parses_a_negated_number_literal_expression_attribute() {
    // Pass 3a: number tokens are always unsigned (docs/MPRX-SPEC.md §2) —
    // a leading `-` is unary negation, not part of the literal. This test
    // replaces Pass 2's `parses_a_negative_decimal_number_literal_expression_attribute`,
    // whose old assertion (a bare `Literal::Number("-3.5")`) no longer holds.
    let element = mesh_parser::parse(r#"<page count={-3.5} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Unary(unary)) => {
            assert_eq!(unary.operator, mesh_syntax::UnaryOperator::Negate);
            match unary.operand.as_ref() {
                mesh_syntax::Expression::Literal(mesh_syntax::Literal::Number(number)) => {
                    assert_eq!(number.value, "3.5");
                }
                other => panic!("expected operand to be a number literal, got {other:?}"),
            }
        }
        other => panic!("expected a unary negation expression, got {other:?}"),
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

#[test]
fn parse_error_implements_display_and_std_error() {
    let error = mesh_parser::parse("<page").expect_err("should fail to parse");

    assert_eq!(error.to_string(), "syntax error");

    fn assert_is_std_error<E: std::error::Error>(_: &E) {}
    assert_is_std_error(&error);
}

#[test]
fn parses_a_logical_not_unary_expression_attribute() {
    let element = mesh_parser::parse(r#"<page disabled={!enabled} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Unary(unary)) => {
            assert_eq!(unary.operator, mesh_syntax::UnaryOperator::Not);
            match unary.operand.as_ref() {
                mesh_syntax::Expression::Reference(reference) => {
                    assert_eq!(reference.name, "enabled");
                }
                other => panic!("expected operand to be a reference, got {other:?}"),
            }
        }
        other => panic!("expected a unary not expression, got {other:?}"),
    }
}

#[test]
fn parses_a_multiplicative_binary_expression_attribute() {
    let element = mesh_parser::parse(r#"<page total={a * b} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(binary)) => {
            assert_eq!(binary.operator, mesh_syntax::BinaryOperator::Mul);
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn parses_every_binary_operator() {
    let cases = [
        (r#"<page v={a / b} />"#, mesh_syntax::BinaryOperator::Div),
        (r#"<page v={a % b} />"#, mesh_syntax::BinaryOperator::Mod),
        (r#"<page v={a + b} />"#, mesh_syntax::BinaryOperator::Add),
        (r#"<page v={a - b} />"#, mesh_syntax::BinaryOperator::Sub),
        (r#"<page v={a < b} />"#, mesh_syntax::BinaryOperator::Lt),
        (r#"<page v={a <= b} />"#, mesh_syntax::BinaryOperator::Le),
        (r#"<page v={a > b} />"#, mesh_syntax::BinaryOperator::Gt),
        (r#"<page v={a >= b} />"#, mesh_syntax::BinaryOperator::Ge),
        (r#"<page v={a == b} />"#, mesh_syntax::BinaryOperator::Eq),
        (r#"<page v={a != b} />"#, mesh_syntax::BinaryOperator::Ne),
        (r#"<page v={a && b} />"#, mesh_syntax::BinaryOperator::And),
        (r#"<page v={a || b} />"#, mesh_syntax::BinaryOperator::Or),
    ];

    for (source, expected_operator) in cases {
        let element =
            mesh_parser::parse(source).unwrap_or_else(|_| panic!("should parse: {source}"));
        match &element.attributes[0].value {
            mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(binary)) => {
                assert_eq!(binary.operator, expected_operator, "source: {source}");
            }
            other => panic!("expected a binary expression for {source}, got {other:?}"),
        }
    }
}

#[test]
fn respects_multiplicative_over_additive_precedence() {
    let element = mesh_parser::parse(r#"<page total={a + b * c} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(outer)) => {
            assert_eq!(outer.operator, mesh_syntax::BinaryOperator::Add);
            match outer.left.as_ref() {
                mesh_syntax::Expression::Reference(reference) => assert_eq!(reference.name, "a"),
                other => panic!("expected left operand to be a reference, got {other:?}"),
            }
            match outer.right.as_ref() {
                mesh_syntax::Expression::Binary(inner) => {
                    assert_eq!(inner.operator, mesh_syntax::BinaryOperator::Mul);
                }
                other => panic!("expected right operand to be a multiplicative binary expression, got {other:?}"),
            }
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn left_associates_repeated_additive_operators() {
    let element = mesh_parser::parse(r#"<page total={a - b - c} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(outer)) => {
            assert_eq!(outer.operator, mesh_syntax::BinaryOperator::Sub);
            match outer.left.as_ref() {
                mesh_syntax::Expression::Binary(inner) => {
                    assert_eq!(inner.operator, mesh_syntax::BinaryOperator::Sub);
                    match (inner.left.as_ref(), inner.right.as_ref()) {
                        (
                            mesh_syntax::Expression::Reference(left),
                            mesh_syntax::Expression::Reference(right),
                        ) => {
                            assert_eq!(left.name, "a");
                            assert_eq!(right.name, "b");
                        }
                        other => panic!("expected inner operands to be references, got {other:?}"),
                    }
                }
                other => {
                    panic!("expected left operand to be a nested binary expression, got {other:?}")
                }
            }
            match outer.right.as_ref() {
                mesh_syntax::Expression::Reference(reference) => assert_eq!(reference.name, "c"),
                other => panic!("expected right operand to be a reference, got {other:?}"),
            }
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn allows_a_unary_operand_inside_a_binary_expression() {
    let element = mesh_parser::parse(r#"<page v={-a * b} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(outer)) => {
            assert_eq!(outer.operator, mesh_syntax::BinaryOperator::Mul);
            match outer.left.as_ref() {
                mesh_syntax::Expression::Unary(unary) => {
                    assert_eq!(unary.operator, mesh_syntax::UnaryOperator::Negate);
                    match unary.operand.as_ref() {
                        mesh_syntax::Expression::Reference(reference) => {
                            assert_eq!(reference.name, "a");
                        }
                        other => panic!("expected unary operand to be a reference, got {other:?}"),
                    }
                }
                other => panic!("expected left operand to be a unary expression, got {other:?}"),
            }
            match outer.right.as_ref() {
                mesh_syntax::Expression::Reference(reference) => assert_eq!(reference.name, "b"),
                other => panic!("expected right operand to be a reference, got {other:?}"),
            }
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn parenthesized_expression_overrides_precedence() {
    let element = mesh_parser::parse(r#"<page total={(a + b) * c} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(outer)) => {
            assert_eq!(outer.operator, mesh_syntax::BinaryOperator::Mul);
            match outer.left.as_ref() {
                mesh_syntax::Expression::Binary(inner) => {
                    assert_eq!(inner.operator, mesh_syntax::BinaryOperator::Add);
                }
                other => {
                    panic!("expected left operand to be the parenthesized addition, got {other:?}")
                }
            }
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }
}

#[test]
fn parses_a_conditional_expression_attribute() {
    let element =
        mesh_parser::parse(r#"<page size={compact ? "sm" : "md"} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Conditional(
            conditional,
        )) => {
            match conditional.condition.as_ref() {
                mesh_syntax::Expression::Reference(reference) => {
                    assert_eq!(reference.name, "compact")
                }
                other => panic!("expected condition to be a reference, got {other:?}"),
            }
            match conditional.consequent.as_ref() {
                mesh_syntax::Expression::Literal(mesh_syntax::Literal::String(s)) => {
                    assert_eq!(s.value, "sm");
                }
                other => panic!("expected consequent to be a string literal, got {other:?}"),
            }
            match conditional.alternate.as_ref() {
                mesh_syntax::Expression::Literal(mesh_syntax::Literal::String(s)) => {
                    assert_eq!(s.value, "md");
                }
                other => panic!("expected alternate to be a string literal, got {other:?}"),
            }
        }
        other => panic!("expected a conditional expression, got {other:?}"),
    }
}

#[test]
fn right_associates_nested_conditional_expressions() {
    let element = mesh_parser::parse(r#"<page v={a ? b : c ? d : e} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Conditional(outer)) => {
            match outer.condition.as_ref() {
                mesh_syntax::Expression::Reference(reference) => assert_eq!(reference.name, "a"),
                other => panic!("expected outer condition to be a reference, got {other:?}"),
            }
            match outer.alternate.as_ref() {
                mesh_syntax::Expression::Conditional(inner) => match inner.condition.as_ref() {
                    mesh_syntax::Expression::Reference(reference) => {
                        assert_eq!(reference.name, "c");
                    }
                    other => panic!("expected inner condition to be a reference, got {other:?}"),
                },
                other => {
                    panic!("expected outer alternate to be a nested conditional, got {other:?}")
                }
            }
        }
        other => panic!("expected a conditional expression, got {other:?}"),
    }
}

#[test]
fn parses_an_empty_array_expression_attribute() {
    let element = mesh_parser::parse(r#"<page items={[]} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Array(array)) => {
            assert!(array.elements.is_empty());
        }
        other => panic!("expected an array expression, got {other:?}"),
    }
}

#[test]
fn parses_an_array_expression_with_elements_attribute() {
    let element = mesh_parser::parse(r#"<page items={[1, 2, 3]} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Array(array)) => {
            assert_eq!(array.elements.len(), 3);
            for (element, expected) in array.elements.iter().zip(["1", "2", "3"]) {
                match element {
                    mesh_syntax::Expression::Literal(mesh_syntax::Literal::Number(number)) => {
                        assert_eq!(number.value, expected);
                    }
                    other => panic!("expected a number literal element, got {other:?}"),
                }
            }
        }
        other => panic!("expected an array expression, got {other:?}"),
    }
}

#[test]
fn parses_an_array_expression_with_a_trailing_comma_attribute() {
    let element = mesh_parser::parse(r#"<page items={[1, 2,]} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Array(array)) => {
            assert_eq!(array.elements.len(), 2);
        }
        other => panic!("expected an array expression, got {other:?}"),
    }
}

#[test]
fn parses_an_empty_object_expression_attribute() {
    let element = mesh_parser::parse(r#"<page data={{}} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(object)) => {
            assert!(object.members.is_empty());
        }
        other => panic!("expected an object expression, got {other:?}"),
    }
}

#[test]
fn parses_an_object_expression_with_an_identifier_key_attribute() {
    let element = mesh_parser::parse(r#"<page data={{ name: "Users" }} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(object)) => {
            assert_eq!(object.members.len(), 1);
            match &object.members[0].key {
                mesh_syntax::ObjectKey::Identifier(name) => assert_eq!(name, "name"),
                other => panic!("expected an identifier key, got {other:?}"),
            }
            match &object.members[0].value {
                mesh_syntax::Expression::Literal(mesh_syntax::Literal::String(s)) => {
                    assert_eq!(s.value, "Users");
                }
                other => panic!("expected a string literal value, got {other:?}"),
            }
        }
        other => panic!("expected an object expression, got {other:?}"),
    }
}

#[test]
fn parses_an_object_expression_with_a_string_key_attribute() {
    let element = mesh_parser::parse(r#"<page data={{ "a-b": 1 }} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(object)) => {
            match &object.members[0].key {
                mesh_syntax::ObjectKey::String(s) => assert_eq!(s.value, "a-b"),
                other => panic!("expected a string key, got {other:?}"),
            }
        }
        other => panic!("expected an object expression, got {other:?}"),
    }
}

#[test]
fn parses_an_object_expression_with_a_trailing_comma_attribute() {
    let element = mesh_parser::parse(r#"<page data={{ a: 1, b: 2, }} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(object)) => {
            assert_eq!(object.members.len(), 2);
        }
        other => panic!("expected an object expression, got {other:?}"),
    }
}

#[test]
fn rejects_a_single_brace_object_literal() {
    // Proves the doubled-brace requirement (docs/MPRX-SPEC.md §5) is
    // actually enforced, not just documented: `data={{ ... }}` is valid,
    // but the single-brace form below is not — `key` parses as a bare
    // reference, then `:` has no valid continuation without a preceding
    // `?`, so this must fail to parse.
    let result = mesh_parser::parse(r#"<page data={ key: "value" } />"#);

    assert!(
        result.is_err(),
        "single-brace object literal should not parse"
    );
}

#[test]
fn parses_a_command_invocation_with_no_arguments_attribute() {
    let element = mesh_parser::parse(r#"<page action={selectUser()} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Command(command)) => {
            assert_eq!(command.command, "selectUser");
            assert!(command.arguments.is_empty());
        }
        other => panic!("expected a command invocation, got {other:?}"),
    }
}

#[test]
fn parses_a_command_invocation_with_an_event_value_argument_attribute() {
    let element =
        mesh_parser::parse(r#"<page action={selectUser($event)} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Command(command)) => {
            assert_eq!(command.command, "selectUser");
            assert_eq!(command.arguments.len(), 1);
            match &command.arguments[0] {
                mesh_syntax::Expression::EventValue(event) => assert_eq!(event.name, "event"),
                other => panic!("expected an event value argument, got {other:?}"),
            }
        }
        other => panic!("expected a command invocation, got {other:?}"),
    }
}

#[test]
fn parses_a_command_invocation_with_multiple_arguments_attribute() {
    let element = mesh_parser::parse(r#"<page action={update(a, b)} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Command(command)) => {
            assert_eq!(command.command, "update");
            assert_eq!(command.arguments.len(), 2);
        }
        other => panic!("expected a command invocation, got {other:?}"),
    }
}

#[test]
fn parses_a_bare_event_value_attribute() {
    let element = mesh_parser::parse(r#"<page handler={$event} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::EventValue(event)) => {
            assert_eq!(event.name, "event");
        }
        other => panic!("expected an event value, got {other:?}"),
    }
}

#[test]
fn parses_a_nested_array_inside_an_object_attribute() {
    let element = mesh_parser::parse(r#"<page data={{ items: [1, 2] }} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(object)) => {
            match &object.members[0].value {
                mesh_syntax::Expression::Array(array) => assert_eq!(array.elements.len(), 2),
                other => panic!("expected an array value, got {other:?}"),
            }
        }
        other => panic!("expected an object expression, got {other:?}"),
    }
}

#[test]
fn parses_a_nested_object_inside_an_array_attribute() {
    let element = mesh_parser::parse(r#"<page items={[{a: 1}]} />"#).expect("should parse");

    match &element.attributes[0].value {
        mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Array(array)) => {
            assert_eq!(array.elements.len(), 1);
            match &array.elements[0] {
                mesh_syntax::Expression::Object(object) => assert_eq!(object.members.len(), 1),
                other => panic!("expected an object element, got {other:?}"),
            }
        }
        other => panic!("expected an array expression, got {other:?}"),
    }
}
