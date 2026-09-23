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
