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
- **Renderer-independent.** A compiled MESH tree carries no assumptions about DOM, Canvas, or hardware. [PORT](https://github.com/ValanceX/Port) decides how each target realizes it.
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

The same checks run in your editor, as you type, through the `mesh-lsp` language server: diagnostics, quick fixes, hover, go to definition and completion. `cargo install --path crates/mesh-lsp`, then follow the [editor setup guide](./docs/guides/editor-setup.md).

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
    ↓  mesh-analysis (checked against the component manifest)
Template (template-v1), one per component
    ↓  a program: a root and its templates
mesh-runtime: render  ──▶  render tree (render-v1)  ──▶  PORT renderer
mesh-runtime: dispatch ◀── handler identifier, payload ◀─┘
    ↓
command intent  ──▶  NEXUS
```

Tree-sitter is an implementation detail. What the rest of Valance consumes is **templates**, which the compiler emits, and the **runtime**, which evaluates them: a host such as NEXUS's adapter renders a program against its state and gets a render tree for a renderer to draw, then turns the events the renderer reports into command intents. The [NEXUS](./docs/guides/integrating-mesh-with-nexus.md) and [PORT](./docs/guides/rendering-mesh-output.md) guides show both sides.

## Inside the repo

```text
mesh/
├── crates/                 Rust implementation
│   ├── mesh-syntax/          syntax nodes and source locations
│   ├── mesh-parser/          Tree-sitter integration, CST → AST
│   ├── mesh-semantic/        semantic model, lowering, structural validation
│   ├── mesh-manifest/        component manifest loading and validation
│   ├── mesh-analysis/        checking templates against a component manifest
│   ├── mesh-template/        the template format (template-v1) and the model fingerprint
│   ├── mesh-compiler/        compile API, diagnostics, and rendering
│   ├── mesh-runtime/         the runtime: render and dispatch, MPRX's one evaluator
│   ├── mesh-runtime-wasm/    the runtime, built for WebAssembly
│   ├── mesh-lsp/             the language server, a client of the compiler
│   ├── mesh-wasm/            the compiler, built for WebAssembly
│   └── mesh-cli/             the `mesh` command-line tool
├── grammar/
│   └── tree-sitter-mprx/     the MPRX Tree-sitter grammar and its highlighting queries
├── editors/                verified editor configurations (Neovim)
└── packages/               npm packages (an npm workspace, published as @valancex/*)
    ├── mesh-compiler/        @valancex/mesh-compiler: the compiler, built for WebAssembly
    ├── mesh-runtime/         @valancex/mesh-runtime: the runtime, built for WebAssembly
    └── mesh-lsp/             @valancex/mesh-lsp: starts the native mesh-lsp
```

## Boundaries

- MESH never depends on [NEXUS](https://github.com/ValanceX/Nexus). The compiler and tooling work on their own.
- The language server reuses `mesh-parser` and `mesh-compiler`. There is never a second parser or type system for editor tooling.
- Other layers (NEXUS, PORT) consume MESH through its formats (templates, render trees, command intents) and the runtime's operations (render, dispatch and declared events), never through Rust compiler internals. MESH evaluates MPRX exactly once, in its runtime; JavaScript only encodes values and carries results, and applies the runtime's patches to the tree it holds (moving parts by key, reading no value).

## Status

**v0.10.0 prepared** (2026-10-09; not yet released: v0.9.0 is the latest release). See the [release notes](./docs/releases/v0.10.md) and [CHANGELOG](./CHANGELOG.md). What works today:

- The MPRX language: elements, attributes, text, `{...}` expressions (literals, references, member access, unary, binary and conditional operators, arrays, objects, command invocations, and `$event`), and event bindings (`on.click={...}`)
- Structural validation: mismatched closing tags, and duplicate attributes or event bindings
- Checking against a component manifest (`mesh check --model`): every component, prop, event, reference and command is resolved, every expression is typed, and every value is checked against its declaration
- rustc-style diagnostics with stable codes, located syntax errors, and did-you-mean suggestions; warnings don't fail the check
- JSON diagnostics (`mesh check --format json`)
- The `mesh-lsp` language server: the compiler's own diagnostics and quick fixes as you type, hover, go to definition, and completion from the manifest, also on files with syntax errors (see its [manual](./docs/manual/mesh-lsp.md))
- Syntax highlighting from the grammar's Tree-sitter queries, and an [editor setup guide](./docs/guides/editor-setup.md) verified in Neovim
- MESH in JavaScript: `@valancex/mesh-compiler` is the compiler in WebAssembly, for Node and browsers, returning exactly what `mesh check --format json` prints (see [Using MESH from JavaScript](./docs/guides/using-mesh-from-javascript.md))
- `mesh-lsp` from npm: `npm install -g @valancex/mesh-lsp` installs the native server for Linux, macOS or Windows
- Compiling to templates: `mesh compile`, and `compile()` in `@valancex/mesh-compiler`, emit a checked component's template (see the [templates manual](./docs/manual/templates.md)); `mesh check-program` and `checkProgram()` check a program of them
- The runtime: `mesh-runtime` in Rust, and `@valancex/mesh-runtime` in Node and browsers, render a program against a snapshot to a render tree, and dispatch an event to a command intent, with MPRX's evaluation specified in [§9.7 of the spec](./docs/MPRX-SPEC.md) (see the [runtime manual](./docs/manual/runtime.md))
- **Update (unreleased):** `update` renders a new snapshot of the same program, reusing every node's props and text whose inputs are unchanged, and returns the patches (`render-patch-v1`: `setProp`, `removeProp`, `setText`, `insert`, `remove`, `move`, and `replace`) that turn the previous tree into the new one, by key. Applying them gives exactly a full render's tree. In JavaScript the module keeps the renders `update` returns, and `render.release()` ends a copy (see the [runtime manual](./docs/manual/runtime.md#update)). It saves a renderer its comparing, and an update keeps every unchanged node as the very same node: on a keyed list of 10,000 items with one changed, 7 expressions evaluated instead of 60,001, and 21 ms in Rust instead of 91 (about 60 ms from JavaScript instead of about 470). What remains is the size of the snapshot, not of the tree, and `updateChanges` removes that too: the host hands over the edits to the snapshot (`set`, `insert`, `remove` at paths, for the render they name by version), and the same update takes 13 ms in Rust and about 22 ms from JavaScript; `diff(previous, next)` computes the edits when the host has only two snapshots (see [Changes](./docs/manual/runtime.md#changes))
- A renderer's contract, settled with PORT: every text a renderer needs is the runtime's (the render tree's `propText` for number, boolean and `null` props), a value is realized natively or as that text and nothing else, and one interaction reaches at most one binding by event resolution ([§9.8.7 and §9.9 of the spec](./docs/MPRX-SPEC.md)), with language-neutral [conformance vectors](./examples/conformance/README.md), and a node-identity contract for structure that varies ([§9.10](./docs/MPRX-SPEC.md), with [identity vectors](./examples/conformance/identity/README.md)); that is a contract only, since MPRX has no conditional or repeated elements yet
- Node identity for a program whose structure varies ([§9.10 of the spec](./docs/MPRX-SPEC.md)): an identity is a site and, at a repeated site, a key the author declares, never a rendered position. The runtime's provisional `mesh-if` and `mesh-each` give conditional and repeated structure, with keys checked in the template and at render, handlers that find a repeated item by identity, and language-neutral [identity vectors](./examples/conformance/identity/README.md) for renderers

Known limitations: MPRX has no presence test for a value that may be absent, and children aren't checked against components. Editor configuration is per editor, and only Neovim's is verified. Composites have a default slot for children and can raise events (both unreleased), but have no named slots. A list holding an absent element can't be rendered or passed as an argument. Every template must be recompiled after any change of meaning to the component model. MESH ships no NEXUS adapter or PORT renderer: those belong to their repositories.

Next: PORT's server rendering and hydration, on v0.7's contract, and NEXUS's adapter, in their own repositories, and a thin VS Code extension over the npm-installed server.

## Learn more

The [documentation index](./docs/README.md) lists everything. The most useful places to start:

- [**Getting started**](./docs/guides/getting-started.md): install `mesh` and check your first file
- [**Writing MPRX**](./docs/guides/writing-mprx.md): the language, with examples and common mistakes
- [**Checking against a component model**](./docs/guides/checking-against-a-component-model.md): manifests, types, and why absence isn't `null`
- [**`mesh` CLI manual**](./docs/manual/mesh-cli.md), [**Diagnostics reference**](./docs/manual/diagnostics.md) and [**Manifest reference**](./docs/manual/manifest.md)
- [**Editor setup**](./docs/guides/editor-setup.md) and the [**`mesh-lsp` manual**](./docs/manual/mesh-lsp.md): MESH in your editor
- [**Using MESH from JavaScript**](./docs/guides/using-mesh-from-javascript.md): check MPRX in Node or a browser with `@valancex/mesh-compiler`
- [**Embedding the compiler**](./docs/guides/embedding-the-compiler.md): use MESH from Rust
- [**Integrating MESH with NEXUS**](./docs/guides/integrating-mesh-with-nexus.md) and [**Rendering MESH output**](./docs/guides/rendering-mesh-output.md): render and dispatch MESH templates, as a host and as a renderer
- [**MPRX Language Spec**](./docs/MPRX-SPEC.md): the exact syntax and semantics
- [**Architecture**](./docs/ARCHITECTURE.md): why MPRX looks the way it does, and how MESH fits into Valance

## Tech

Rust, Tree-sitter, LSP, and WASM where useful, with thin TypeScript packages for distribution.

## License

MIT. See [LICENSE](./LICENSE).
