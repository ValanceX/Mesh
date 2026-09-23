# MESH

**A UI language you can trust a machine to write.**

MESH is the UI language toolchain of [Valance](https://github.com/ValanceX). It's built around **MPRX** (MeshExpr), a small declarative language that looks like the HTML and JSX you already know. MPRX is deliberately limited: it has no arbitrary code, no side effects, and no business logic. That limit is what makes it safe.

Because MPRX is small, MESH can parse, validate, and compile every UI before it runs. That matters most when the UI comes from somewhere you don't fully control, such as an AI model, a remote server, or a plugin.

```xml
<user-card
  user={user}
  compact={layout.compact}
  on.select={selectUser($event)} />
```

> MPRX is the language. MESH is the system that understands, validates, and compiles it.

## Why MESH

- **Familiar syntax.** Tags, attributes, and `{expressions}` work the way you'd expect if you've used HTML, JSX, Vue, or Svelte.
- **Checked before it runs.** Invalid syntax, mismatched tags, and duplicate bindings are caught at compile time, not in front of users.
- **Safe for generated UI.** MPRX can only express structure, bindings, simple expressions, and *intent*. Something like `selectUser($event)` is a request for NEXUS to handle, not code that MESH runs.
- **Renderer-independent.** A compiled MESH tree carries no assumptions about DOM, Canvas, or hardware. [PORT](https://github.com/ValanceX/Port) decides how it's drawn.
- **One brain for compiler and editor.** The language server reuses the compiler itself, so your editor and your build always agree.

## Try it

MESH ships a native CLI. From the repo root:

```console
$ cargo run -p mesh-cli -- check examples/page.mprx
no errors
```

Mistakes are reported with their location:

```console
$ cargo run -p mesh-cli -- check broken.mprx
error: mismatched closing tag: opened with "user-card", closed with "user-kard" (0..94)
```

## How it works

```text
MPRX source
    ↓  Tree-sitter grammar
Concrete syntax tree
    ↓  mesh-parser
MESH AST
    ↓  mesh-semantic (lowering + validation)
MESH Semantic IR
    ↓
NEXUS runtime  +  PORT renderer
```

Tree-sitter is an implementation detail. What the rest of Valance consumes is the **Semantic IR**, a stable, renderer-independent description of the UI.

## Inside the repo

```text
mesh/
├── crates/                 Rust implementation
│   ├── mesh-syntax/          syntax nodes and source locations
│   ├── mesh-parser/          Tree-sitter integration, CST → AST
│   ├── mesh-semantic/        semantic model, lowering, structural validation
│   ├── mesh-compiler/        diagnostics, type checking, codegen
│   ├── mesh-lsp/             language server (reuses the compiler)
│   └── mesh-cli/             the `mesh` command-line tool
├── grammar/
│   └── tree-sitter-mprx/     the MPRX Tree-sitter grammar
└── packages/               thin npm wrappers around the Rust/WASM build
    ├── mesh-language/        @valence/mesh-language
    ├── mesh-compiler/        @valence/mesh-compiler
    └── mesh-lsp/             @valence/mesh-lsp
```

## Boundaries

- MESH never depends on [NEXUS](https://github.com/ValanceX/Nexus). The compiler and tooling work on their own.
- The language server reuses `mesh-parser` and `mesh-compiler`. There is never a second parser or type system for editor tooling.
- Other layers (NEXUS, PORT, other runtimes) consume MESH through the Semantic IR, never through Rust compiler internals.

## Status

**v0.1 in active development.** What works today:

- Elements, attributes, strings, and text
- `{...}` expressions in attributes and content: literals, references, member access, unary, binary and conditional operators, arrays, objects, command invocations, and `$event`
- Event bindings (`on.click={...}`)
- Structural validation: mismatched closing tags, and duplicate attributes or event bindings
- The `mesh check` CLI

Next up: polishing the canonical example and diagnostic rendering. After that comes component-aware type checking.

## Learn more

- [**MPRX Language Spec**](./docs/MPRX-SPEC.md): the exact syntax, with what's shipped and what's planned
- [**Architecture**](./docs/ARCHITECTURE.md): why MPRX looks the way it does, and how MESH fits into Valance

## Tech

Rust, Tree-sitter, LSP, and WASM where useful, with thin TypeScript packages for distribution.

## License

MIT. See [LICENSE](./LICENSE).
