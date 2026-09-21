#[test]
fn lowers_a_self_closing_element() {
    let ast = mesh_syntax::Element {
        name: "page".to_string(),
        attributes: vec![mesh_syntax::Attribute {
            name: "title".to_string(),
            value: mesh_syntax::StringLiteral {
                value: "Users".to_string(),
                span: mesh_syntax::Span {
                    start_byte: 0,
                    end_byte: 0,
                },
            },
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
    assert_eq!(ir.attributes[0].value, "Users");
    assert!(ir.children.is_empty());
}
