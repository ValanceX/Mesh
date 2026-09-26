# MESH Architecture

This document explains *why* MESH and MPRX are designed the way they are. For the exact syntax, see the [MPRX Language Spec](./MPRX-SPEC.md). The sibling repos, [NEXUS](https://github.com/ValanceX/Nexus) and [PORT](https://github.com/ValanceX/Port), each have their own architecture doc. For the big picture, start at the [ValanceX organization page](https://github.com/ValanceX).

## The short version

Valance splits a UI app into three questions:

| Question | Answered by |
|---|---|
| What should the UI *mean*? | **MESH** |
| What should the app *do*? | NEXUS |
| Where and how should it *appear*? | PORT |

MESH owns the first one. It's a small language toolchain, with a grammar, parser, semantic model, compiler, and language server, all built around one language: **MPRX**.

> MPRX is the language. MESH is the system that understands, compiles, validates, transforms, and executes it.

## How source becomes UI

MESH is more than an AST. Tree-sitter parses the source, but its tree is only an intermediate step. What the rest of Valance actually consumes is **templates**, which the compiler emits, and the **runtime**, which evaluates them.

```text
MPRX Source → Tree-sitter → Concrete Syntax Tree → MESH Semantic IR
  → Analysis against the component manifest → Template (one per component)

Program (a root and its templates) + snapshot → MESH runtime → render tree → PORT
Handler identifier + payload + the render   → MESH runtime → command intent → NEXUS
```

- **NEXUS** holds the state and runs the commands. Its adapter is a MESH **host**: it renders a program against a snapshot of its state, keeps each render, and dispatches the events a renderer reports with the render they came from. See [Integrating MESH with NEXUS](./guides/integrating-mesh-with-nexus.md).
- **PORT** realizes the UI on a target. Today its renderers get render trees: primitive components with final values, and nothing to evaluate. See [Rendering MESH output](./guides/rendering-mesh-output.md).

Whatever changes, MESH stays **renderer-independent**. It never assumes a DOM, a canvas, or a particular device.

## MPRX: a deliberately small language

MPRX (MeshExpr, "Mesh Expression") is a declarative UI language with HTML/XML-like syntax. If you've written HTML, JSX, Angular templates, Vue, or Svelte, it will look familiar, though it doesn't copy any of their semantics wholesale.

```xml
<user-card
  user={user}
  compact={layout.compact}
  on.select={selectUser($event)} />
```

Expressions go in `{...}`. There's no Angular-style `[foo]` or `(click)` syntax.

### What MPRX is for, and what it isn't

MPRX isn't TypeScript inside XML, and it isn't a general-purpose language, a replacement for Effect, or a business-logic DSL.

| MPRX expresses | MPRX never contains |
|---|---|
| UI structure and component composition | HTTP calls or database access |
| Bindings and simple derived expressions | Business rules or infrastructure logic |
| References to UI, app, and environment state | Arbitrary TypeScript or service imports |
| Command and event *intent* | Side effects or hardware operations |
| Styling references and control flow | |

**Why so strict?** Every limit is something the compiler can check. A UI that can't make network calls can't make unexpected ones. And since expressions are side-effect free, evaluating one can never change anything.

### The expression model

The starting set of expression nodes is small on purpose:

```text
Literal, Reference, MemberAccess, UnaryExpression, BinaryExpression,
ConditionalExpression, ArrayExpression, ObjectExpression, CommandInvocation
```

```xml
<page title="Users">
  <text>{user.name}</text>
  <avatar src={user.avatar} alt={user.name} size={compact ? "sm" : "md"} />
  <button disabled={!user.active} on.click={selectUser($event)}>Select</button>
</page>
```

We don't add an expression form just because the parser *could* support it. New syntax needs a concrete reason.

### Commands are intent, not code

```xml
<user-card user={user} on.select={selectUser($event)} />
```

This doesn't call a function. It records a request:

```text
CommandInvocation
├── command: selectUser
└── arguments
    └── $event
```

NEXUS decides what `selectUser` actually does. MESH only represents the invocation and never executes application logic.

## Static verification

A core goal is catching mistakes at compile time. If you write `<user-card user={usr} />` and `usr` doesn't exist, the compiler should tell you before anything renders.

Over time the compiler will understand component names, props, local variables, references, member access, expression types, commands and their argument types, event values, bindings, control flow, and eventually slots.

> MPRX should feel as light as a template while giving you compiler-level checking.

## Built for generated UI

This is where the constraints pay off. When an AI model (or any other untrusted source) produces UI, it goes through the same checks as hand-written code:

```text
AI → MPRX source → Parser → MESH AST → Semantic validation → Template → Runtime → Render tree → PORT
```

Valance never blindly executes generated UI. The compiler rejects invalid syntax, unknown components or properties, bad references, malformed command calls, type errors, and unsupported constructs.

## Implementation

### Why Rust

Compilers are a good fit for Rust. It gives us a strong type system, explicit invariants, efficient tree manipulation, room for incremental compilation and parallelism, native CLI tooling, WASM distribution, and first-class LSP support.

```text
mesh/
├── crates/
│   ├── mesh-syntax/
│   ├── mesh-parser/
│   ├── mesh-semantic/
│   ├── mesh-manifest/
│   ├── mesh-analysis/
│   ├── mesh-template/
│   ├── mesh-compiler/
│   ├── mesh-runtime/
│   ├── mesh-runtime-wasm/
│   ├── mesh-wasm/
│   ├── mesh-lsp/
│   └── mesh-cli/
├── grammar/
│   └── tree-sitter-mprx/     the grammar, and its highlighting queries
├── editors/                  verified editor configurations
├── packages/                 @valancex/mesh-compiler, @valancex/mesh-runtime, @valancex/mesh-lsp
└── Cargo.toml
```

Crate boundaries can change as we learn more.

### Who does what

- **mesh-language**: the MPRX grammar, Tree-sitter parser, syntax nodes, AST types, source locations, expression grammar, basic semantic model, and AST traversal.
- **mesh-compiler**: semantic analysis, type checking, component and binding resolution, template compilation, diagnostics, transformation, optimization, and code generation.
- **mesh-template**: the template format, `template-v1`, and the model fingerprint. It has no parser, so the runtime can depend on it.
- **mesh-runtime**: MPRX's one evaluator. It validates a program, its model and the host's values, renders a tree, and dispatches an event to a command intent. It accepts templates, never source. `mesh-runtime-wasm` builds it for WebAssembly, and `@valancex/mesh-runtime` wraps that for JavaScript, encoding values and nothing more.
- **mesh-lsp**: diagnostics and quick fixes, hover, go-to-definition, and completion. It is **a client of the compiler**, which stays the single semantic authority: its diagnostics are the compiler's own, and it has no parser or type system of its own, so the editor and the build can't disagree. Its npm package only launches it.

  An open document has three kinds of state, and they never mix. **Canonical state** is exactly what `mesh_compiler::compile_with` returns for its text: its diagnostics, and for a file that parses, its IR and analysis. **Editor recovery state** exists only for a file with a syntax error: `mesh_compiler::editor::recover` keeps the parts that parse and analyzes them with the same analysis code, so hover and go-to-definition still work there. It has no diagnostics, so it can never be published or mistaken for the compiler's verdict. **Presentation** turns either into protocol messages (ranges, hover text, quick fixes) and adds nothing of its own.

- **Highlighting** comes from the grammar's Tree-sitter queries, which editors load directly. They colour text and decide nothing: no MESH feature reads a query capture, so they may be approximate where the compiler may not.

### Language-neutral formats

The runtime's inputs and outputs are JSON documents with published schemas, so no consumer depends on Rust:
- a **template** ([`template-v1`](../schemas/template-v1.schema.json)) is one checked component, every name resolved, no values. It's compatible with a runtime by its format version and its model fingerprint, never by the compiler's version;
- a **render tree** ([`render-v1`](../schemas/render-v1.schema.json)) is what a renderer draws;
- a **command intent** is what a host receives;
- **runtime diagnostics** ([`runtime-diagnostics-v1`](../schemas/runtime-diagnostics-v1.schema.json)) say why a render or dispatch failed.

The Semantic IR itself stays internal to the compiler. The [templates manual](./manual/templates.md) and the [runtime manual](./manual/runtime.md) are the contracts.

## Direction: semantics, not implementations

Valance is a semantic platform with progressively specialized target implementations. MESH's share of that is **template semantics and semantic correctness**: it says what the UI means, and PORT decides how each target realizes it.

- **Abstract what has stable meaning.** Component structure, bindings, identity, interaction intent, relationships between elements, accessibility intent, update semantics and portable style concepts belong in MESH. DOM internals, widget toolkits, compositors, GPU APIs and target layout machinery don't, even as a lowest-common-denominator model. `on.click` is interaction intent, not a DOM event.
- **Describe guarantees, not mechanisms.** What MESH hands on should say "this element has stable identity" or "these updates may be batched", not "this node is type 17". Then MESH can change its internals without breaking a PORT. Before changing a published format, ask: *did the semantic contract change, or only the implementation?* Only the first must be visible to consumers, and explicitly.
- **Keep useful information.** The semantic output should keep what a PORT could use to make better decisions, not flatten everything into a generic node early. New information should improve realization, never become a requirement for correctness, so an older PORT that ignores it stays correct.
- **No single frozen IR.** MESH output is one representation among several (NEXUS, MESH, PORT, target), each with its own scope. The contract stabilizes meaning, not every data structure. Today that output is templates and render trees; it's expected to grow as the first PORT shows what it needs.
- **Targets describe themselves.** MESH never keeps an encyclopedia of platforms. A PORT reports what its target can guarantee, and MESH and its tooling may reason about those capabilities.
- **Styling follows the same split.** MESH may analyze portable style semantics (`padding`, `display: flex`). Target-specific behavior is explicit, as a rule condition like `@target linux { … }` alongside `@media (width < 600px) { … }`, never by pretending every target styles alike. PORT realizes the result.
- **Inspectable.** A developer should eventually be able to trace an element from MPRX through the MESH output into what a PORT made of it. MESH's part is keeping source locations and stable identity available for that.
- **Escape hatches are explicit.** Descending to PORT capabilities, target primitives or native APIs has to be visibly marked as leaving the portable layer, never done by quietly letting platform details into MPRX.

None of this is implemented yet beyond templates and render trees. It's the direction later formats and language features should follow.

## The rules

These invariants hold for everything in this repo:

1. **MESH never performs hardware operations.**
2. **A MESH tree is renderer-independent.**
3. **Generated UI must pass structural and semantic validation before rendering.**
4. **MPRX does not contain arbitrary TypeScript.**
5. **MPRX expressions are side-effect free.**
6. **The MESH compiler and tooling must not require NEXUS.**
7. **The LSP and compiler share one language implementation.**
8. **Component references and bindings should be statically verifiable.**
9. **MPRX has one evaluator: the Rust runtime.** Nothing outside it evaluates an expression, turns a value into text, coerces or normalizes a value, fills in for absence, decides whether a component is a composite, resolves a handler identifier, or judges a host's value. Everything else only carries values, and a renderer only draws them.
10. **Only what checked runs.** The compiler emits a template only from source with no errors, and the runtime accepts only templates, validates every program before running it, and fails closed on any value that doesn't fit.
11. **Renderers see values, never MPRX.** A render tree holds primitive component names, final prop values, text, handler identifiers and keys, and nothing else. Keys and handler identifiers are opaque: they don't reveal those names to anyone without the templates. But they're not secret.
12. **The host is checked, not trusted.** The model, the templates, the snapshot, every payload, every handler identifier and every render are validated on every call.
13. **Dependencies point one way.** No MESH crate or package depends on NEXUS, PORT or Effect. NEXUS's adapter depends on `@valancex/mesh-runtime`, and PORT's renderers on MESH's render-tree types. NEXUS and PORT don't depend on each other. Any program can be a host, and MESH's own tests use one.
14. **Abstract semantics, not implementations.** Nothing target-specific enters MESH unless an application marks it as target-specific explicitly.
15. **Formats carry guarantees, not internals.** Optional information in MESH output improves realization but is never needed for correctness.
