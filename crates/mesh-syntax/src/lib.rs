//! Syntax nodes, AST types, and source locations for MPRX.
//!
//! This crate owns the shape of the MPRX AST. It does not parse source text
//! (see `mesh-parser`) and does not perform semantic analysis (see
//! `mesh-semantic`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Text>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: StringLiteral,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub value: String,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_a_self_closing_element() {
        let element = Element {
            name: "page".to_string(),
            attributes: vec![Attribute {
                name: "title".to_string(),
                value: StringLiteral {
                    value: "Users".to_string(),
                    span: Span {
                        start_byte: 12,
                        end_byte: 19,
                    },
                },
                span: Span {
                    start_byte: 6,
                    end_byte: 19,
                },
            }],
            children: vec![],
            span: Span {
                start_byte: 0,
                end_byte: 22,
            },
        };

        assert_eq!(element.name, "page");
        assert_eq!(element.attributes[0].name, "title");
        assert_eq!(element.attributes[0].value.value, "Users");
    }
}
