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

MESH is more than an AST. Tree-sitter parses the source, but its tree is only an intermediate step. What the rest of Valance actually consumes is the **MESH Semantic IR**.

```text
MPRX Source → Tree-sitter → Concrete Syntax Tree → MESH Semantic AST/IR
  → Semantic Analysis → Validation/Transformation → Runtime/PORT
```

Long-term, the Rust compiler produces the IR, and runtimes for different languages consume it:

```text
MPRX → Rust MESH Compiler → MESH Semantic IR → Target Adapter/Runtime → NEXUS + PORT
```

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
AI → MPRX source → Parser → MESH AST → Semantic validation → Compilation → PORT
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
│   ├── mesh-compiler/
│   ├── mesh-lsp/
│   └── mesh-cli/
├── grammar/
│   └── tree-sitter-mprx/     the grammar, and its highlighting queries
├── editors/                  verified editor configurations
└── Cargo.toml
```

Crate boundaries can change as we learn more.

### Who does what

- **mesh-language**: the MPRX grammar, Tree-sitter parser, syntax nodes, AST types, source locations, expression grammar, basic semantic model, and AST traversal.
- **mesh-compiler**: semantic analysis, type checking, component and binding resolution, template compilation, diagnostics, transformation, optimization, and code generation.
- **mesh-lsp**: diagnostics and quick fixes, hover, go-to-definition, and completion. It is **a client of the compiler**, which stays the single semantic authority: its diagnostics are the compiler's own, and it has no parser or type system of its own, so the editor and the build can't disagree. Its npm package only launches it.

  An open document has three kinds of state, and they never mix. **Canonical state** is exactly what `mesh_compiler::compile_with` returns for its text: its diagnostics, and for a file that parses, its IR and analysis. **Editor recovery state** exists only for a file with a syntax error: `mesh_compiler::editor::recover` keeps the parts that parse and analyzes them with the same analysis code, so hover and go-to-definition still work there. It has no diagnostics, so it can never be published or mistaken for the compiler's verdict. **Presentation** turns either into protocol messages (ranges, hover text, quick fixes) and adds nothing of its own.

- **Highlighting** comes from the grammar's Tree-sitter queries, which editors load directly. They colour text and decide nothing: no MESH feature reads a query capture, so they may be approximate where the compiler may not.

### A language-neutral IR (long-term)

The Semantic IR describes UI without tying it to any runtime language:

```text
Component("user-card")
  .bind("user", Reference("user"))
  .bind("compact", Member("layout", "compact"))
  .event("select", Command("selectUser", EventValue))
```

```text
MPRX → Rust MESH Compiler → MESH Semantic IR → {TypeScript, WASM, Native} Runtime → PORT
```

We're intentionally *not* locking down a binary or serialized IR format yet. Semantic correctness and runtime boundaries come first.

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
