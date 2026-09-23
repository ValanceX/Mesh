# tree-sitter-mprx

Tree-sitter grammar for MPRX (MeshExpr), the declarative UI language used by
Valance. Consumed by the `mesh-parser` crate to produce a concrete syntax
tree, which `mesh-parser` then lowers into the `mesh-syntax` AST.

Not yet implemented — this is a placeholder for the grammar package
(`grammar.js`, generated `src/parser.c`, node bindings, etc.) described in
[`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md).
