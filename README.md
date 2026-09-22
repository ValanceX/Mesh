# MESH

MESH is the UI language ecosystem of [VALENCE](https://github.com/valence-ui). It owns **MPRX**
(MeshExpr) — a declarative, HTML/XML-like UI language — and everything that
understands it: the Tree-sitter grammar, the parser, the semantic model, the
compiler, and the language server.

> MPRX is the language. MESH is the system that understands, compiles,
> validates, transforms, and executes that language.

MESH is **renderer-independent** and has **no dependency on
[`nexus`](https://github.com/valence-ui/nexus)**. It talks to the rest of
VALENCE only through a stable Semantic IR.

```text
MPRX Source
    ↓
Tree-sitter
    ↓
Concrete Syntax Tree
    ↓
MESH Semantic AST / IR
    ↓
Semantic Analysis
    ↓
Validation / Transformation
    ↓
Runtime / PORT
```

## Why MPRX exists

MPRX looks familiar to anyone coming from HTML/JSX/Angular/Vue/Svelte, but it
is a real, intentionally constrained language — not "TypeScript inside XML."
It expresses UI structure, bindings, simple derived expressions, and
command/event intent. It does **not** express business logic, side effects,
HTTP calls, or arbitrary TypeScript. This is what lets AI-generated MPRX be
parsed, statically validated, and compiled before it is ever executed — see
[`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the full rationale.

```xml
<user-card
  user={user}
  compact={layout.compact}
  on.select={selectUser($event)} />
```

## Layout

```text
mesh/
├── crates/            Rust implementation
│   ├── mesh-syntax/       syntax nodes, source locations
│   ├── mesh-parser/       Tree-sitter integration, CST → AST
│   ├── mesh-semantic/     semantic model, component/binding resolution
│   ├── mesh-compiler/     type checking, diagnostics, codegen
│   ├── mesh-lsp/          LSP server (reuses the compiler, not a second implementation)
│   └── mesh-cli/          native CLI
├── grammar/
│   └── tree-sitter-mprx/  MPRX Tree-sitter grammar
└── packages/           published npm wrapper packages
    ├── mesh-language/     @valence/mesh-language
    ├── mesh-compiler/     @valence/mesh-compiler
    └── mesh-lsp/          @valence/mesh-lsp
```

The Tree-sitter CST is **not** the application's final UI model — it feeds
the MESH Semantic AST/IR, which is what the compiler, LSP, and runtimes
actually consume.

## Dependency boundary

- MESH must not depend on NEXUS.
- `mesh-lsp` must reuse `mesh-parser`/`mesh-compiler` — no second parser or
  type system for editor tooling.
- Consumers (NEXUS, PORT, other language runtimes) receive a MESH Semantic
  IR through a stable contract, never Rust compiler internals directly.

## Tech

Rust, Tree-sitter, LSP, WASM where useful, for the compiler crates. The
`packages/` directory holds thin published TypeScript wrapper packages
around the compiled Rust/WASM artifacts.

## Status

The Tree-sitter grammar and parser are implemented and under active
development (v0.1 Pass 1, Pass 2, and Pass 3a — Unary/Binary/Conditional
expressions — shipped). See
[`docs/MPRX-SPEC.md`](./docs/MPRX-SPEC.md) for the current language spec
and what's implemented vs. still planned, and
[`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the system rationale.

## License

MIT — see [LICENSE](./LICENSE).
