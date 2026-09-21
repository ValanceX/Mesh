//! Parses MPRX source text via the `tree-sitter-mprx` grammar and lowers the
//! resulting concrete syntax tree into the `mesh-syntax` AST.
//!
//! The Tree-sitter CST is an implementation detail of this crate — it is
//! never exposed as the application's final UI model.
