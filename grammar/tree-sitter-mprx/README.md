# tree-sitter-mprx

The Tree-sitter grammar for MPRX (MeshExpr), the declarative UI language of
MESH. It has two users:

- **`mesh-parser`**, which parses MPRX with it and lowers the concrete
  syntax tree into the `mesh-syntax` AST. The grammar decides only what
  parses; everything MPRX means is decided by the MESH compiler.
- **Editors**, through `queries/highlights.scm`, which colours MPRX with
  nvim-treesitter's capture names. The queries only colour text: no MESH
  feature reads them. The [editor setup
  guide](../../docs/guides/editor-setup.md) installs them.

## Layout

- `grammar.js`: the grammar. `src/` is generated from it and committed;
  after changing it, run `npx tree-sitter generate` here (CI checks that
  `src/` is up to date).
- `test/corpus/`: parse tests, run by `npm test`.
- `queries/highlights.scm` and `test/captures/`: the highlighting query,
  and sample files with every capture it makes on them. MPRX has no
  comments, so Tree-sitter's own highlight tests can't be used; the Rust
  test in `bindings/rust/highlight_tests.rs` compares the captures instead,
  and `MESH_BLESS=1 cargo test -p tree-sitter-mprx` rewrites them after a
  deliberate change.

## Bindings

Only the Rust binding (`bindings/rust`) is built and tested: `mesh-parser`
uses it, and CI runs its tests. The C, Go, Node, Python and Swift bindings
under `bindings/` are the Tree-sitter CLI's templates, kept as generated;
nothing builds or tests them, and the grammar isn't published to any
registry. Editors build the parser from `src/` (see the editor setup
guide).

## Working on it

```console
$ npm ci --ignore-scripts && npm rebuild tree-sitter-cli
$ npm test
$ cargo test -p tree-sitter-mprx
```
