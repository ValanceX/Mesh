//! Semantic model for MPRX.
//!
//! For v0.1, this crate builds the Semantic IR from the `mesh-syntax` AST
//! and performs structural pass-through only. It does not yet resolve
//! component/prop/binding references against an external component model —
//! that requires a typed component model that doesn't exist yet.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Text>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub value: String,
}

pub fn lower(ast: &mesh_syntax::Element) -> Element {
    Element {
        name: ast.name.clone(),
        attributes: ast
            .attributes
            .iter()
            .map(|a| Attribute { name: a.name.clone(), value: a.value.value.clone() })
            .collect(),
        children: ast.children.iter().map(|t| Text { value: t.value.clone() }).collect(),
    }
}
