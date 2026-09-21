# MESH — Architecture

This is the MESH-relevant excerpt of the VALENCE architecture. MESH is one of
three independent repos (`nexus`, `mesh`, `port`) that make up VALENCE; see
each repo's own docs for its slice, or the full design doc kept in the
[VALENCE namespace folder](https://github.com/valence-ui) for the complete
picture.

## Role

MESH is the UI language/toolchain/runtime ecosystem.

> MPRX is the language. MESH is the system that understands, compiles,
> validates, transforms, and executes that language.

MESH is NOT merely an AST representation. Tree-sitter is an implementation
detail used by MESH, not the application's final UI model.

```text
MPRX Source → Tree-sitter → Concrete Syntax Tree → MESH Semantic AST/IR
  → Semantic Analysis → Validation/Transformation → Runtime/PORT
```

Eventually:

```text
MPRX → Rust MESH Compiler → MESH Semantic IR → Target Adapter/Runtime → NEXUS + PORT
```

MESH must remain renderer-independent.

## MPRX

MPRX means MeshExpr/Mesh Expression — a declarative UI language with an
HTML/XML-like syntax, familiar to developers from HTML, Angular templates,
JSX, Vue, or Svelte, but not simply copying their semantics.

```xml
<user-card
  user={user}
  compact={layout.compact}
  on.select={selectUser($event)} />
```

Expressions use `{...}`. Do not use Angular's `[foo]` / `(click)` syntax.

### Philosophy

MPRX is NOT: TypeScript inside XML, a general-purpose programming language,
a replacement for Effect, or a business-logic DSL.

MPRX should express: UI structure, component composition, bindings, simple
derived expressions, UI/application/environment state references,
command/event intent, styling references, control flow where appropriate.

MPRX should NOT contain: HTTP calls, database access, infrastructure logic,
arbitrary side effects, business rules, arbitrary TypeScript, imports of
application services, hardware operations. Expressions are side-effect free.

### Expression Model

Deliberately constrained initial node set:

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

Do not allow arbitrary TypeScript expressions just because the parser can
technically support them.

### Commands

```xml
<user-card user={user} on.select={selectUser($event)} />
```

parses to:

```text
CommandInvocation
├── command: selectUser
└── arguments
    └── $event
```

NEXUS (a separate repo) resolves the command's actual behavior. MESH only
represents the invocation — it never executes application logic.

## Static Verification

A major goal of MESH is strong static verification: the compiler should
verify MPRX references against the component model (e.g. flag
`<user-card user={usr} />` at compile time if `usr` doesn't exist). It
should understand component names, props, local variables, references,
member access, expression types, commands, command argument types, event
values, bindings, control flow, and eventually slots.

> MPRX should feel lightweight like a template while providing
> compiler-level verification of its semantics.

## Rust Implementation

```text
mesh/
├── crates/
│   ├── mesh-syntax/
│   ├── mesh-parser/
│   ├── mesh-semantic/
│   ├── mesh-compiler/
│   ├── mesh-lsp/
│   └── mesh-cli/
├── grammar/
│   └── tree-sitter-mprx/
└── Cargo.toml
```

Reasons for Rust: compiler-style workloads, strong type system, explicit
invariants, efficient AST/IR manipulation, incremental compilation,
parallelism, native CLI tooling, WASM distribution, LSP support. Crate
boundaries can evolve with implementation experience.

## Package Responsibilities

- **mesh-language** — MPRX grammar, Tree-sitter parser, syntax nodes, AST
  types, source locations, expression grammar, basic semantic model, AST
  traversal.
- **mesh-compiler** — semantic analysis, type checking, component
  resolution, binding resolution, template compilation, diagnostics,
  transformation, optimization, code generation.
- **mesh-lsp** — LSP protocol, completion, hover, diagnostics,
  go-to-definition, references, rename, document symbols, formatting. Must
  reuse the same parser/compiler infrastructure as mesh-compiler — never a
  second parser/type system for editor tooling.

## Language-Neutral Semantic IR (long-term)

```text
Component("user-card")
  .bind("user", Reference("user"))
  .bind("compact", Member("layout", "compact"))
  .event("select", Command("selectUser", EventValue))
```

```text
MPRX → Rust MESH Compiler → MESH Semantic IR → {TypeScript, WASM, Native} Runtime → PORT
```

Do not prematurely lock down a binary/serialized IR format — establish
semantic correctness and runtime boundaries first.

## AI UI Generation

```text
AI → MPRX source → Parser → MESH AST → Semantic validation → Compilation → PORT
```

The framework must never blindly execute arbitrary AI-generated UI. The
compiler rejects invalid syntax, components, properties, references,
command calls, types, and unsupported constructs.

## Invariants relevant to MESH

1. MESH never performs hardware operations.
2. A MESH tree is renderer-independent.
3. Generated UI must pass structural and semantic validation before rendering.
4. MPRX does not contain arbitrary TypeScript.
5. MPRX expressions are side-effect free.
6. MESH compiler/tooling must not require NEXUS.
7. LSP and compiler share the same language implementation.
8. Component references and bindings should be statically verifiable.
