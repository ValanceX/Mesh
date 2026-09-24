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
- **Checked before it runs.** Invalid syntax is caught at compile time, not in front of users. Against a component manifest, so is every unknown component, prop, event, name or command, and every value of the wrong type.
- **Safe for generated UI.** MPRX can only express structure, bindings, simple expressions, and *intent*. Something like `selectUser($event)` is a request for NEXUS to handle, not code that MESH runs.
- **Renderer-independent.** A compiled MESH tree carries no assumptions about DOM, Canvas, or hardware. [PORT](https://github.com/ValanceX/Port) decides how it's drawn.
- **One brain for compiler and editor.** The language server reuses the compiler itself, so your editor and your build always agree.

## Try it

MESH ships a native CLI. Install it from a clone of this repo and check the canonical example:

```console
$ cargo install --path crates/mesh-cli
$ mesh check examples/user-card.mprx
no errors
```

(Or skip installing and run `cargo run -p mesh-cli -- check <file>` from the repo root.)

Give it a component manifest, and it checks what the file means, too:

```console
$ mesh check --model examples/components.json examples/users-page.mprx
no errors
```

Mistakes are reported with their location:

```console
$ mesh check examples/fixtures/fail/mismatched-closing-tag.mprx
error[mismatched-closing-tag]: mismatched closing tag: opened with "title", closed with "heading"
 --> examples/fixtures/fail/mismatched-closing-tag.mprx:2:3
  |
2 |   <title>Users</heading>
  |   ^^^^^^^^^^^^^^^^^^^^^^
```

Warnings (such as a duplicate attribute) are printed the same way but don't fail the check.

New to MESH? The [getting-started guide](./docs/guides/getting-started.md) walks through it step by step.

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
│   ├── mesh-manifest/        component manifest loading and validation
│   ├── mesh-analysis/        checking templates against a component manifest
│   ├── mesh-compiler/        compile API, diagnostics, and rendering
│   ├── mesh-lsp/             language server (a placeholder)
│   └── mesh-cli/             the `mesh` command-line tool
├── grammar/
│   └── tree-sitter-mprx/     the MPRX Tree-sitter grammar
└── packages/               npm wrappers for the Rust/WASM build (placeholders)
    ├── mesh-language/        @valance/mesh-language
    ├── mesh-compiler/        @valance/mesh-compiler
    └── mesh-lsp/             @valance/mesh-lsp
```

## Boundaries

- MESH never depends on [NEXUS](https://github.com/ValanceX/Nexus). The compiler and tooling work on their own.
- The language server reuses `mesh-parser` and `mesh-compiler`. There is never a second parser or type system for editor tooling.
- Other layers (NEXUS, PORT, other runtimes) consume MESH through the Semantic IR, never through Rust compiler internals.

## Status

**v0.2.0 released** (2026-09-24). See the [release notes](./docs/releases/v0.2.md) and [CHANGELOG](./CHANGELOG.md). What works today:

- The MPRX language: elements, attributes, text, `{...}` expressions (literals, references, member access, unary, binary and conditional operators, arrays, objects, command invocations, and `$event`), and event bindings (`on.click={...}`)
- Structural validation: mismatched closing tags, and duplicate attributes or event bindings
- Checking against a component manifest (`mesh check --model`): every component, prop, event, reference and command is resolved, every expression is typed, and every value is checked against its declaration
- rustc-style diagnostics with stable codes, located syntax errors, and did-you-mean suggestions; warnings don't fail the check
- JSON diagnostics (`mesh check --format json`)

Known limitations: MPRX has no presence test for a value that may be absent, and children aren't checked against components. The language server and npm packages are placeholders.

Next up: editor support through `mesh-lsp`.

## Learn more

The [documentation index](./docs/README.md) lists everything. The most useful places to start:

- [**Getting started**](./docs/guides/getting-started.md): install `mesh` and check your first file
- [**Writing MPRX**](./docs/guides/writing-mprx.md): the language, with examples and common mistakes
- [**Checking against a component model**](./docs/guides/checking-against-a-component-model.md): manifests, types, and why absence isn't `null`
- [**`mesh` CLI manual**](./docs/manual/mesh-cli.md), [**Diagnostics reference**](./docs/manual/diagnostics.md) and [**Manifest reference**](./docs/manual/manifest.md)
- [**Embedding the compiler**](./docs/guides/embedding-the-compiler.md): use MESH from Rust
- [**MPRX Language Spec**](./docs/MPRX-SPEC.md): the exact syntax and semantics
- [**Architecture**](./docs/ARCHITECTURE.md): why MPRX looks the way it does, and how MESH fits into Valance

## Tech

Rust, Tree-sitter, LSP, and WASM where useful, with thin TypeScript packages for distribution.

## License

MIT. See [LICENSE](./LICENSE).
