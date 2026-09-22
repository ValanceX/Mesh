#[test]
fn lowers_a_self_closing_element_with_a_string_attribute() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
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
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast);

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
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast);

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
        attributes: vec![],
        children: vec![mesh_syntax::Child::Expression(mesh_syntax::Expression::Reference(
            mesh_syntax::Reference {
                name: "user".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            },
        ))],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast);

    assert_eq!(
        ir.children[0],
        mesh_semantic::Child::Expression(mesh_semantic::Expression::Reference("user".to_string()))
    );
}

#[test]
fn lowers_boolean_and_null_literals() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
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
        children: vec![],
        span: mesh_syntax::Span {
            start_byte: 0,
            end_byte: 0,
        },
    };

    let ir = mesh_semantic::lower(&ast);

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
