#[test]
fn lowers_a_self_closing_element_with_a_string_attribute() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "title".to_string(),
            value: mesh_syntax::AttributeValue::String(mesh_syntax::StringLiteral {
                value: "Users".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            }),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(ir.name, "page");
    assert_eq!(ir.attributes.len(), 1);
    assert_eq!(ir.attributes[0].name, "title");
    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::String("Users".to_string())
    );
    assert!(ir.children.is_empty());
}

#[test]
fn lowers_a_member_access_expression_attribute() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "user".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::MemberAccess(
                mesh_syntax::MemberAccess {
                    object: Box::new(mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                        name: "user".to_string(),
                        span: mesh_syntax::Span {
                            start_byte: 0,
                            end_byte: 0,
                        },
                    })),
                    property: "name".to_string(),
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::MemberAccess {
            object: Box::new(mesh_semantic::Expression::Reference("user".to_string())),
            property: "name".to_string(),
        })
    );
}

#[test]
fn lowers_an_expression_child() {
    let ast = mesh_syntax::Element {
        name: "title".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![],
        children: vec![mesh_syntax::Child::Expression(
            mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "user".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            }),
        )],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Expression(mesh_semantic::Expression::Reference("user".to_string()))
    );
}

#[test]
fn lowers_boolean_and_null_literals() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![
            mesh_syntax::Attribute {
                name: "disabled".to_string(),
                value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
                    mesh_syntax::Literal::Boolean(mesh_syntax::BooleanLiteral {
                        value: true,
                        span: mesh_syntax::Span {
                            start_byte: 0,
                            end_byte: 0,
                        },
                    }),
                )),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            },
            mesh_syntax::Attribute {
                name: "value".to_string(),
                value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Literal(
                    mesh_syntax::Literal::Null(mesh_syntax::NullLiteral {
                        span: mesh_syntax::Span {
                            start_byte: 0,
                            end_byte: 0,
                        },
                    }),
                )),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            },
        ],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

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
fn lowers_a_unary_expression() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "disabled".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Unary(
                mesh_syntax::UnaryExpression {
                    operator: mesh_syntax::UnaryOperator::Not,
                    operand: Box::new(mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                        name: "disabled".to_string(),
                        span: mesh_syntax::Span {
                            start_byte: 0,
                            end_byte: 0,
                        },
                    })),
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Unary {
            operator: mesh_syntax::UnaryOperator::Not,
            operand: Box::new(mesh_semantic::Expression::Reference("disabled".to_string())),
        })
    );
}

#[test]
fn lowers_a_binary_expression_preserving_nested_precedence() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "total".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Binary(
                mesh_syntax::BinaryExpression {
                    operator: mesh_syntax::BinaryOperator::Add,
                    left: Box::new(mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                        name: "a".to_string(),
                        span: mesh_syntax::Span {
                            start_byte: 0,
                            end_byte: 0,
                        },
                    })),
                    right: Box::new(mesh_syntax::Expression::Binary(
                        mesh_syntax::BinaryExpression {
                            operator: mesh_syntax::BinaryOperator::Mul,
                            left: Box::new(mesh_syntax::Expression::Reference(
                                mesh_syntax::Reference {
                                    name: "b".to_string(),
                                    span: mesh_syntax::Span {
                                        start_byte: 0,
                                        end_byte: 0,
                                    },
                                },
                            )),
                            right: Box::new(mesh_syntax::Expression::Reference(
                                mesh_syntax::Reference {
                                    name: "c".to_string(),
                                    span: mesh_syntax::Span {
                                        start_byte: 0,
                                        end_byte: 0,
                                    },
                                },
                            )),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        },
                    )),
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

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
fn lowers_a_conditional_expression() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "size".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Conditional(
                mesh_syntax::ConditionalExpression {
                    condition: Box::new(mesh_syntax::Expression::Reference(
                        mesh_syntax::Reference {
                            name: "compact".to_string(),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        },
                    )),
                    consequent: Box::new(mesh_syntax::Expression::Literal(
                        mesh_syntax::Literal::String(mesh_syntax::StringLiteral {
                            value: "sm".to_string(),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        }),
                    )),
                    alternate: Box::new(mesh_syntax::Expression::Literal(
                        mesh_syntax::Literal::String(mesh_syntax::StringLiteral {
                            value: "md".to_string(),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        }),
                    )),
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

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
fn lowers_an_array_expression() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "items".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Array(
                mesh_syntax::ArrayExpression {
                    elements: vec![
                        mesh_syntax::Expression::Literal(mesh_syntax::Literal::Number(
                            mesh_syntax::NumberLiteral {
                                value: "1".to_string(),
                                span: mesh_syntax::Span {
                                    start_byte: 0,
                                    end_byte: 0,
                                },
                            },
                        )),
                        mesh_syntax::Expression::Literal(mesh_syntax::Literal::Number(
                            mesh_syntax::NumberLiteral {
                                value: "2".to_string(),
                                span: mesh_syntax::Span {
                                    start_byte: 0,
                                    end_byte: 0,
                                },
                            },
                        )),
                    ],
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Array(vec![
            mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number("1".to_string())),
            mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number("2".to_string())),
        ]))
    );
}

#[test]
fn lowers_an_object_expression_flattening_the_key() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "data".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Object(
                mesh_syntax::ObjectExpression {
                    members: vec![
                        mesh_syntax::ObjectMember {
                            key: mesh_syntax::ObjectKey::Identifier("name".to_string()),
                            value: mesh_syntax::Expression::Literal(mesh_syntax::Literal::String(
                                mesh_syntax::StringLiteral {
                                    value: "Users".to_string(),
                                    span: mesh_syntax::Span {
                                        start_byte: 0,
                                        end_byte: 0,
                                    },
                                },
                            )),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        },
                        mesh_syntax::ObjectMember {
                            key: mesh_syntax::ObjectKey::String(mesh_syntax::StringLiteral {
                                value: "a-b".to_string(),
                                span: mesh_syntax::Span {
                                    start_byte: 0,
                                    end_byte: 0,
                                },
                            }),
                            value: mesh_syntax::Expression::Literal(mesh_syntax::Literal::Number(
                                mesh_syntax::NumberLiteral {
                                    value: "1".to_string(),
                                    span: mesh_syntax::Span {
                                        start_byte: 0,
                                        end_byte: 0,
                                    },
                                },
                            )),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        },
                    ],
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Object(vec![
            mesh_semantic::ObjectMember {
                key: "name".to_string(),
                value: mesh_semantic::Expression::Literal(mesh_semantic::Literal::String(
                    "Users".to_string()
                )),
            },
            mesh_semantic::ObjectMember {
                key: "a-b".to_string(),
                value: mesh_semantic::Expression::Literal(mesh_semantic::Literal::Number(
                    "1".to_string()
                )),
            },
        ]))
    );
}

#[test]
fn lowers_a_command_invocation() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "action".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::Command(
                mesh_syntax::CommandInvocation {
                    command: "selectUser".to_string(),
                    arguments: vec![mesh_syntax::Expression::EventValue(
                        mesh_syntax::EventValue {
                            name: "event".to_string(),
                            span: mesh_syntax::Span {
                                start_byte: 0,
                                end_byte: 0,
                            },
                        },
                    )],
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::Command {
            command: "selectUser".to_string(),
            arguments: vec![mesh_semantic::Expression::EventValue("event".to_string())],
        })
    );
}

#[test]
fn lowers_an_event_value() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "handler".to_string(),
            value: mesh_syntax::AttributeValue::Expression(mesh_syntax::Expression::EventValue(
                mesh_syntax::EventValue {
                    name: "event".to_string(),
                    span: mesh_syntax::Span {
                        start_byte: 0,
                        end_byte: 0,
                    },
                },
            )),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.attributes[0].value,
        mesh_semantic::AttributeValue::Expression(mesh_semantic::Expression::EventValue(
            "event".to_string()
        ))
    );
}

#[test]
fn omits_whitespace_only_text_children_from_the_ir() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("div".to_string()),
        attributes: vec![],
        event_bindings: vec![],
        children: vec![
            mesh_syntax::Child::Text(mesh_syntax::Text {
                value: "\n    ".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            }),
            mesh_syntax::Child::Text(mesh_syntax::Text {
                value: "Hello".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            }),
        ],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(ir.children.len(), 1);
    assert_eq!(ir.children[0], mesh_semantic::Child::Text("Hello".to_string()));
}

#[test]
fn preserves_meaningful_text_verbatim_including_internal_whitespace() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("div".to_string()),
        attributes: vec![],
        event_bindings: vec![],
        children: vec![mesh_syntax::Child::Text(mesh_syntax::Text {
            value: "Hello     world".to_string(),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        })],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Text("Hello     world".to_string())
    );
}

#[test]
fn lowers_a_nested_element() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("div".to_string()),
        attributes: vec![],
        event_bindings: vec![],
        children: vec![mesh_syntax::Child::Element(Box::new(mesh_syntax::Element {
            name: "span".to_string(),
            closing_name: Some("span".to_string()),
            attributes: vec![],
            event_bindings: vec![],
            children: vec![mesh_syntax::Child::Text(mesh_syntax::Text {
                value: "A".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            })],
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }))],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast).ir.expect("should produce IR");

    assert_eq!(ir.children.len(), 1);
    match &ir.children[0] {
        mesh_semantic::Child::Element(inner) => {
            assert_eq!(inner.name, "span");
            assert_eq!(inner.children[0], mesh_semantic::Child::Text("A".to_string()));
        }
        other => panic!("expected an element child, got {other:?}"),
    }
}

#[test]
fn lowers_an_event_binding() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![mesh_syntax::EventBinding {
            name: "click".to_string(),
            handler: mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "handler".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            }),
            span: mesh_syntax::Span {
                start_byte: 0,
                end_byte: 0,
            },
        }],
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    match &element.event_bindings[0].handler {
        mesh_semantic::Expression::Reference(reference) => assert_eq!(reference, "handler"),
        other => panic!("expected a reference expression handler, got {other:?}"),
    }
}

#[test]
fn duplicate_attributes_keep_the_last_occurrence_and_warn_about_earlier_ones() {
    fn string_attribute(name: &str, value: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::Attribute {
        mesh_syntax::Attribute {
            name: name.to_string(),
            value: mesh_syntax::AttributeValue::String(mesh_syntax::StringLiteral {
                value: value.to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![
            string_attribute("class", "a", 0, 1),
            string_attribute("id", "x", 2, 3),
            string_attribute("class", "b", 4, 5),
            string_attribute("id", "y", 6, 7),
        ],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 8 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    assert_eq!(element.attributes.len(), 2);
    assert_eq!(element.attributes[0].name, "class");
    assert_eq!(element.attributes[1].name, "id");

    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 0, end_byte: 1 });
    assert_eq!(result.diagnostics[1].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[1].span, mesh_syntax::Span { start_byte: 2, end_byte: 3 });
}

#[test]
fn surviving_attributes_are_ordered_by_their_winning_occurrence_not_first_encountered_order() {
    fn string_attribute(name: &str, value: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::Attribute {
        mesh_syntax::Attribute {
            name: name.to_string(),
            value: mesh_syntax::AttributeValue::String(mesh_syntax::StringLiteral {
                value: value.to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    // Source order: class="a" id="x" class="b" title="y" id="z"
    //
    // First-encountered order would be [class, id, title] (class and id
    // are both first seen before title). Winning-occurrence order is
    // [class, title, id] instead, because id's *winning* occurrence
    // (pos 4) comes after title's only occurrence (pos 3) — this is the
    // case that actually distinguishes the two orderings.
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![
            string_attribute("class", "a", 0, 1),
            string_attribute("id", "x", 2, 3),
            string_attribute("class", "b", 4, 5),
            string_attribute("title", "y", 6, 7),
            string_attribute("id", "z", 8, 9),
        ],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 10 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    assert_eq!(element.attributes.len(), 3);
    assert_eq!(element.attributes[0].name, "class");
    match &element.attributes[0].value {
        mesh_semantic::AttributeValue::String(value) => assert_eq!(value, "b"),
        other => panic!("expected a string attribute value, got {other:?}"),
    }
    assert_eq!(element.attributes[1].name, "title");
    assert_eq!(element.attributes[2].name, "id");
    match &element.attributes[2].value {
        mesh_semantic::AttributeValue::String(value) => assert_eq!(value, "z"),
        other => panic!("expected a string attribute value, got {other:?}"),
    }

    // Diagnostics are ordered by shadowed-occurrence source position:
    // the shadowed "class" (pos 0) precedes the shadowed "id" (pos 2).
    assert_eq!(result.diagnostics.len(), 2);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 0, end_byte: 1 });
    assert_eq!(result.diagnostics[1].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[1].span, mesh_syntax::Span { start_byte: 2, end_byte: 3 });
}

#[test]
fn duplicate_event_bindings_keep_the_last_occurrence_and_warn_about_earlier_ones() {
    fn event_binding(name: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::EventBinding {
        mesh_syntax::EventBinding {
            name: name.to_string(),
            handler: mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "handler".to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![
            event_binding("click", 0, 1),
            event_binding("click", 2, 3),
        ],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 4 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 0, end_byte: 1 });
}

#[test]
fn an_attribute_and_an_event_binding_with_the_same_name_do_not_collide() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![mesh_syntax::Attribute {
            name: "click".to_string(),
            value: mesh_syntax::AttributeValue::String(mesh_syntax::StringLiteral {
                value: "x".to_string(),
                span: mesh_syntax::Span { start_byte: 0, end_byte: 1 },
            }),
            span: mesh_syntax::Span { start_byte: 0, end_byte: 1 },
        }],
        event_bindings: vec![mesh_syntax::EventBinding {
            name: "click".to_string(),
            handler: mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "handler".to_string(),
                span: mesh_syntax::Span { start_byte: 2, end_byte: 3 },
            }),
            span: mesh_syntax::Span { start_byte: 2, end_byte: 3 },
        }],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 4 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    assert_eq!(element.attributes.len(), 1);
    assert_eq!(element.event_bindings.len(), 1);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn a_nested_child_s_duplicate_event_binding_diagnostic_propagates_to_the_parent_result() {
    fn event_binding(name: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::EventBinding {
        mesh_syntax::EventBinding {
            name: name.to_string(),
            handler: mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "handler".to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    let child = mesh_syntax::Element {
        name: "button".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![
            event_binding("click", 10, 11),
            event_binding("click", 12, 13),
        ],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 9, end_byte: 14 },
    };

    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![],
        children: vec![mesh_syntax::Child::Element(Box::new(child))],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 15 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should lower");

    // The outer element has no duplicates of its own; its single
    // diagnostic is the one recursively threaded up from the nested
    // child, per the "Diagnostic ordering (global invariant)" rule:
    // the parent's own diagnostics (none, here) precede its children's.
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Warning);
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 10, end_byte: 11 });

    assert_eq!(element.children.len(), 1);
    let mesh_semantic::Child::Element(child) = &element.children[0] else {
        panic!("expected a nested element child");
    };
    assert_eq!(child.event_bindings.len(), 1);
}

#[test]
fn mismatched_closing_tag_produces_a_non_fatal_error_diagnostic_and_still_lowers() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("span".to_string()),
        attributes: vec![],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 10 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("should still lower despite the mismatch");

    assert_eq!(element.name, "div");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Error);
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 0, end_byte: 10 });
}

#[test]
fn self_closing_elements_are_never_tag_mismatch_checked() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: None,
        attributes: vec![],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 5 },
    };

    let result = mesh_semantic::lower(&ast);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn matching_closing_tag_produces_no_diagnostics() {
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("div".to_string()),
        attributes: vec![],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 10 },
    };

    let result = mesh_semantic::lower(&ast);
    assert!(result.diagnostics.is_empty());
}

#[test]
fn diagnostics_are_ordered_depth_first_pre_order_across_all_three_rules() {
    fn string_attribute(name: &str, value: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::Attribute {
        mesh_syntax::Attribute {
            name: name.to_string(),
            value: mesh_syntax::AttributeValue::String(mesh_syntax::StringLiteral {
                value: value.to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    fn event_binding(name: &str, start_byte: usize, end_byte: usize) -> mesh_syntax::EventBinding {
        mesh_syntax::EventBinding {
            name: name.to_string(),
            handler: mesh_syntax::Expression::Reference(mesh_syntax::Reference {
                name: "handler".to_string(),
                span: mesh_syntax::Span { start_byte, end_byte },
            }),
            span: mesh_syntax::Span { start_byte, end_byte },
        }
    }

    // Child: one duplicate attribute only (id="x" shadowed by id="y").
    let child = mesh_syntax::Element {
        name: "button".to_string(),
        closing_name: None,
        attributes: vec![
            string_attribute("id", "x", 10, 11),
            string_attribute("id", "y", 12, 13),
        ],
        event_bindings: vec![],
        children: vec![],
        span: mesh_syntax::Span { start_byte: 9, end_byte: 14 },
    };

    // Outer: duplicate attribute + duplicate event binding + mismatched
    // closing tag, and one nested child (above) with its own duplicate
    // attribute.
    let ast = mesh_syntax::Element {
        name: "div".to_string(),
        closing_name: Some("span".to_string()),
        attributes: vec![
            string_attribute("class", "a", 0, 1),
            string_attribute("class", "b", 2, 3),
        ],
        event_bindings: vec![
            event_binding("click", 4, 5),
            event_binding("click", 6, 7),
        ],
        children: vec![mesh_syntax::Child::Element(Box::new(child))],
        span: mesh_syntax::Span { start_byte: 0, end_byte: 20 },
    };

    let result = mesh_semantic::lower(&ast);
    let element = result.ir.expect("all three diagnostic categories are non-fatal");

    // IR is still fully populated despite 4 diagnostics.
    assert_eq!(element.name, "div");
    assert_eq!(element.attributes.len(), 1);
    assert_eq!(element.attributes[0].name, "class");
    assert_eq!(element.event_bindings.len(), 1);
    assert_eq!(element.event_bindings[0].name, "click");
    assert_eq!(element.children.len(), 1);
    let mesh_semantic::Child::Element(child) = &element.children[0] else {
        panic!("expected a nested element child");
    };
    assert_eq!(child.attributes.len(), 1);
    assert_eq!(child.attributes[0].name, "id");

    // Exact order: (1) outer's own duplicate-attribute warning, (2)
    // outer's own duplicate-event-binding warning, (3) outer's own
    // tag-mismatch error, (4) the child's duplicate-attribute warning
    // — per the depth-first, pre-order rule: a parent's own diagnostics
    // precede its children's. This ordering is never produced by a sort:
    // it falls directly out of lower_element's traversal (attribute
    // dedup -> event-binding dedup -> tag-mismatch check -> children
    // recursion, per Step 5 below), so asserting exact vector position
    // here is what proves no post-lowering sort exists.
    assert_eq!(result.diagnostics.len(), 4);

    // (1) Outer's own duplicate-attribute warning — shadowed occurrence
    // of "class" at (0, 1). Message format matches dedupe_last_wins
    // (Task 2 Step 7): `duplicate {kind} {name:?}: this occurrence is
    // shadowed by a later one`.
    assert_eq!(result.diagnostics[0].severity, mesh_syntax::Severity::Warning);
    assert_eq!(
        result.diagnostics[0].message,
        "duplicate attribute \"class\": this occurrence is shadowed by a later one"
    );
    assert_eq!(result.diagnostics[0].span, mesh_syntax::Span { start_byte: 0, end_byte: 1 });

    // (2) Outer's own duplicate-event-binding warning — shadowed
    // occurrence of "click" at (4, 5). Same dedupe_last_wins format,
    // kind = "event binding".
    assert_eq!(result.diagnostics[1].severity, mesh_syntax::Severity::Warning);
    assert_eq!(
        result.diagnostics[1].message,
        "duplicate event binding \"click\": this occurrence is shadowed by a later one"
    );
    assert_eq!(result.diagnostics[1].span, mesh_syntax::Span { start_byte: 4, end_byte: 5 });

    // (3) Outer's own tag-mismatch error, spanning the whole opening
    // element (ast.span). Message format matches the tag-mismatch check
    // (Task 3 Step 5): `mismatched closing tag: opened with {name:?},
    // closed with {closing_name:?}`.
    assert_eq!(result.diagnostics[2].severity, mesh_syntax::Severity::Error);
    assert_eq!(
        result.diagnostics[2].message,
        "mismatched closing tag: opened with \"div\", closed with \"span\""
    );
    assert_eq!(result.diagnostics[2].span, mesh_syntax::Span { start_byte: 0, end_byte: 20 });

    // (4) The nested child's own duplicate-attribute warning — shadowed
    // occurrence of "id" at (10, 11), propagated up through
    // lower_element's recursion into children.
    assert_eq!(result.diagnostics[3].severity, mesh_syntax::Severity::Warning);
    assert_eq!(
        result.diagnostics[3].message,
        "duplicate attribute \"id\": this occurrence is shadowed by a later one"
    );
    assert_eq!(result.diagnostics[3].span, mesh_syntax::Span { start_byte: 10, end_byte: 11 });
}
