# MESH Rust Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 6 applicable findings from the 2026-09-22 rust-skills audit of `mesh`'s hand-written crates (mesh-syntax, mesh-parser, mesh-semantic, mesh-compiler, mesh-cli), without changing any currently-passing test's observable behavior.

**Architecture:** Each crate gets exactly one task bundling its findings, ordered leaf-first (mesh-syntax has no workspace deps, mesh-cli depends on everything) so each task's `cargo test -p <crate>` only ever exercises code already fixed by an earlier task. The lint-centralization finding lands last, as a final gate over everything the other five tasks touched.

**Tech Stack:** Rust 2021, `thiserror` (new dependency, mesh-parser only), `tempfile` (new dev-dependency, mesh-cli only). No other new dependencies.

**Spec:** No separate spec file — this plan implements the 7 findings reported by the rust-skills audit run in this conversation on 2026-09-22 against `/home/jackmiller/Projects/@Valance/mesh`. Finding #7 (no property-based/fuzz testing) is explicitly out of scope for this plan: there is no concrete property worth testing against the current Pass 1/2 grammar, and building that infrastructure now would be speculative ahead of Pass 3's expression-precedence work, which is what would actually justify it. The 6 findings this plan fixes, in the order the tasks below address them:

1. Silent wildcard (`_`) match arms in `mesh-parser`'s lowering functions treat unmatched Tree-sitter node kinds as an assumed default instead of a bug, even though the assumed-default kinds (`"reference"`, `"number_literal"`) are real, nameable node types.
2. No public item in any of the 5 hand-written crates has a `///` doc comment (only crate-level `//!` headers exist), including the fallible `mesh_parser::parse`, which has no `# Errors` section.
3. `ParseError` (mesh-parser) and `Diagnostic` (mesh-compiler) implement neither `std::fmt::Display` nor `std::error::Error`; no `thiserror`/`anyhow` exists anywhere in the workspace.
4. No `[workspace.lints]` table exists in the root `Cargo.toml`; clippy strictness (`-D warnings`) is enforced only via CI's CLI flags, not centrally.
5. `Severity` (mesh-compiler) is a single-variant enum that the roadmap's own Pass 4 diagnostics work is expected to grow (e.g. a `Warning` variant) — it should be `#[non_exhaustive]` now to avoid a breaking change later. This does NOT apply to the AST/IR enums (`Expression`, `Literal`, `Child`, `AttributeValue` in both mesh-syntax and mesh-semantic) — those are meant to stay exhaustively matched by design.
6. `mesh-cli/tests/check.rs`'s `check_reports_an_error_for_invalid_source` test writes a fixture to `std::env::temp_dir()` with a fixed filename and never cleans it up.

## Global Constraints

- Every task must end with `cargo test --workspace` still passing in full — not just the touched crate's own tests. Run it at the end of every task, not just the crate-scoped test.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` must both stay clean after every task (verify both at the end of every task, not just the last one).
- No new dependency beyond `thiserror` (Task 3, mesh-parser only) and `tempfile` (Task 5, mesh-cli dev-dependency only) — do not introduce `anyhow` or anything else.
- Do not touch `mesh-lsp` (out of v0.1 scope per the roadmap) or the `grammar/tree-sitter-mprx` crate (generated/vendored tree-sitter bindings, not hand-written project code).
- Do not change any currently-passing test's assertions or any function's existing public name/signature — only add doc comments, add trait impls, add `#[non_exhaustive]`, replace wildcard match arms with explicit ones, and add new tests. `mesh_parser::parse`, `mesh_compiler::compile`, `mesh_semantic::lower`, and every public struct/enum field keep their current names and types.
- Doc comments document the type/function itself (one summary paragraph, `# Errors` where applicable) — do not add per-field `///` comments where the field name is already self-evident (e.g. `start_byte: usize`); this keeps the doc pass proportionate rather than exhaustive-to-the-field.

---

### Task 1: mesh-syntax — doc comments on every public type

**Files:**
- Modify: `crates/mesh-syntax/src/lib.rs`

**Interfaces:**
- Consumes: nothing (mesh-syntax has no workspace dependencies)
- Produces: nothing new — pure documentation, no signature changes. Later tasks don't depend on anything from this one.

- [ ] **Step 1: Add doc comments to every public item**

Replace the full contents of `crates/mesh-syntax/src/lib.rs` with:

```rust
//! Syntax nodes, AST types, and source locations for MPRX.
//!
//! This crate owns the shape of the MPRX AST. It does not parse source text
//! (see `mesh-parser`) and does not perform semantic analysis (see
//! `mesh-semantic`).

/// A byte-offset range into the original source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

/// An MPRX element: `<name attr={...}>children</name>` or
/// `<name attr={...} />`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Child>,
    pub span: Span,
}

/// A single `name=value` or `name={expression}` attribute on an [`Element`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
    pub span: Span,
}

/// A quoted string literal, with escape sequences already decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

/// A run of literal text inside an element's children (not inside an
/// `{expression}` block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub value: String,
    pub span: Span,
}

/// A number literal. Stored as the raw source text (not parsed to `f64`)
/// so the AST stays a lossless representation of what was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLiteral {
    pub value: String,
    pub span: Span,
}

/// A `true` or `false` literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanLiteral {
    pub value: bool,
    pub span: Span,
}

/// A `null` literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullLiteral {
    pub span: Span,
}

/// A literal value inside an `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    String(StringLiteral),
    Number(NumberLiteral),
    Boolean(BooleanLiteral),
    Null(NullLiteral),
}

/// A bare identifier reference, e.g. `{user}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub span: Span,
}

/// A property access on another expression, e.g. `{user.name}` or the
/// chained `{a.b.c}` (whose `object` is itself a `MemberAccess`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub object: Box<Expression>,
    pub property: String,
    pub span: Span,
}

/// An expression inside an `{...}` block: a literal, a reference, or a
/// member access. Pass 3 will extend this with unary/binary/conditional/
/// array/object/command/event-value variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Reference(Reference),
    MemberAccess(MemberAccess),
}

/// The value side of an [`Attribute`]: either a plain quoted string or an
/// `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    String(StringLiteral),
    Expression(Expression),
}

/// One child of an [`Element`]: either literal [`Text`] or an
/// `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(Text),
    Expression(Expression),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_a_self_closing_element() {
        let element = Element {
            name: "page".to_string(),
            attributes: vec![Attribute {
                name: "title".to_string(),
                value: AttributeValue::String(StringLiteral {
                    value: "Users".to_string(),
                    span: Span {
                        start_byte: 12,
                        end_byte: 19,
                    },
                }),
                span: Span {
                    start_byte: 6,
                    end_byte: 19,
                },
            }],
            children: vec![],
            span: Span {
                start_byte: 0,
                end_byte: 22,
            },
        };

        assert_eq!(element.name, "page");
        assert_eq!(element.attributes[0].name, "title");
        match &element.attributes[0].value {
            AttributeValue::String(literal) => assert_eq!(literal.value, "Users"),
            other => panic!("expected a string attribute value, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_element_with_an_expression_attribute() {
        let element = Element {
            name: "page".to_string(),
            attributes: vec![Attribute {
                name: "title".to_string(),
                value: AttributeValue::Expression(Expression::MemberAccess(MemberAccess {
                    object: Box::new(Expression::Reference(Reference {
                        name: "user".to_string(),
                        span: Span {
                            start_byte: 0,
                            end_byte: 4,
                        },
                    })),
                    property: "name".to_string(),
                    span: Span {
                        start_byte: 0,
                        end_byte: 9,
                    },
                })),
                span: Span {
                    start_byte: 0,
                    end_byte: 9,
                },
            }],
            children: vec![],
            span: Span {
                start_byte: 0,
                end_byte: 10,
            },
        };

        match &element.attributes[0].value {
            AttributeValue::Expression(Expression::MemberAccess(member)) => {
                assert_eq!(member.property, "name");
                match member.object.as_ref() {
                    Expression::Reference(reference) => assert_eq!(reference.name, "user"),
                    other => panic!("expected a reference, got {other:?}"),
                }
            }
            other => panic!("expected a member access expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_element_with_an_expression_child() {
        let element = Element {
            name: "title".to_string(),
            attributes: vec![],
            children: vec![Child::Expression(Expression::Reference(Reference {
                name: "user".to_string(),
                span: Span {
                    start_byte: 7,
                    end_byte: 11,
                },
            }))],
            span: Span {
                start_byte: 0,
                end_byte: 20,
            },
        };

        match &element.children[0] {
            Child::Expression(Expression::Reference(reference)) => {
                assert_eq!(reference.name, "user");
            }
            other => panic!("expected an expression child, got {other:?}"),
        }
    }
}
```

Only the doc comments (`///`) above the type declarations are new — every type, field, derive, and the entire `tests` module are unchanged from the current file.

- [ ] **Step 2: Verify it builds and docs generate cleanly**

Run: `cargo doc -p mesh-syntax --no-deps 2>&1 | tail -20`
Expected: no warnings (in particular, no `missing_docs`-style complaints — this crate doesn't `#![warn(missing_docs)]`, so this is just confirming `cargo doc` itself doesn't choke on the new comments).

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: PASS — identical test count/results to before this task (this task changes no test, only adds doc comments).

- [ ] **Step 4: Confirm fmt and clippy are still clean**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both exit 0 with no output.

- [ ] **Step 5: Commit**

```bash
git add crates/mesh-syntax/src/lib.rs
git commit -m "docs(mesh-syntax): document every public AST type"
```

---

### Task 2: mesh-semantic — doc comments on every public type and `lower`

**Files:**
- Modify: `crates/mesh-semantic/src/lib.rs`

**Interfaces:**
- Consumes: nothing new (mesh-semantic already depends on mesh-syntax; Task 1 didn't change any type mesh-semantic uses)
- Produces: nothing new — pure documentation, no signature changes.

- [ ] **Step 1: Add doc comments to every public item**

Replace the full contents of `crates/mesh-semantic/src/lib.rs` with:

```rust
//! Semantic model for MPRX.
//!
//! For v0.1, this crate builds the Semantic IR from the `mesh-syntax` AST
//! and performs structural pass-through only. It does not yet resolve
//! component/prop/binding references against an external component model —
//! that requires a typed component model that doesn't exist yet.

/// The Semantic IR form of an [`mesh_syntax::Element`] — structurally
/// identical to the AST for v0.1, since no resolution happens yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Child>,
}

/// The Semantic IR form of an [`mesh_syntax::Attribute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
}

/// The Semantic IR form of an [`mesh_syntax::AttributeValue`]. Unlike the
/// AST's `String(StringLiteral)`, the string case here is a plain `String`
/// — source spans are AST-only and don't carry into the IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    String(String),
    Expression(Expression),
}

/// The Semantic IR form of an [`mesh_syntax::Child`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(String),
    Expression(Expression),
}

/// The Semantic IR form of an [`mesh_syntax::Literal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
}

/// The Semantic IR form of an [`mesh_syntax::Expression`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Reference(String),
    MemberAccess {
        object: Box<Expression>,
        property: String,
    },
}

/// Lowers an AST [`mesh_syntax::Element`] into its Semantic IR form.
///
/// For v0.1 this is a structural pass-through (drops source spans, copies
/// everything else) — it does not yet resolve references against a
/// component model.
pub fn lower(ast: &mesh_syntax::Element) -> Element {
    Element {
        name: ast.name.clone(),
        attributes: ast.attributes.iter().map(lower_attribute).collect(),
        children: ast.children.iter().map(lower_child).collect(),
    }
}

fn lower_attribute(attribute: &mesh_syntax::Attribute) -> Attribute {
    Attribute {
        name: attribute.name.clone(),
        value: lower_attribute_value(&attribute.value),
    }
}

fn lower_attribute_value(value: &mesh_syntax::AttributeValue) -> AttributeValue {
    match value {
        mesh_syntax::AttributeValue::String(literal) => {
            AttributeValue::String(literal.value.clone())
        }
        mesh_syntax::AttributeValue::Expression(expression) => {
            AttributeValue::Expression(lower_expression(expression))
        }
    }
}

fn lower_child(child: &mesh_syntax::Child) -> Child {
    match child {
        mesh_syntax::Child::Text(text) => Child::Text(text.value.clone()),
        mesh_syntax::Child::Expression(expression) => {
            Child::Expression(lower_expression(expression))
        }
    }
}

fn lower_expression(expression: &mesh_syntax::Expression) -> Expression {
    match expression {
        mesh_syntax::Expression::Literal(literal) => Expression::Literal(lower_literal(literal)),
        mesh_syntax::Expression::Reference(reference) => {
            Expression::Reference(reference.name.clone())
        }
        mesh_syntax::Expression::MemberAccess(member) => Expression::MemberAccess {
            object: Box::new(lower_expression(&member.object)),
            property: member.property.clone(),
        },
    }
}

fn lower_literal(literal: &mesh_syntax::Literal) -> Literal {
    match literal {
        mesh_syntax::Literal::String(s) => Literal::String(s.value.clone()),
        mesh_syntax::Literal::Number(n) => Literal::Number(n.value.clone()),
        mesh_syntax::Literal::Boolean(b) => Literal::Boolean(b.value),
        mesh_syntax::Literal::Null(_) => Literal::Null,
    }
}
```

Only the doc comments above `Element`, `Attribute`, `AttributeValue`, `Child`, `Literal`, `Expression`, and `lower` are new — every type, field, and function body is unchanged.

- [ ] **Step 2: Verify docs generate cleanly**

Run: `cargo doc -p mesh-semantic --no-deps 2>&1 | tail -20`
Expected: no warnings.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: PASS — identical results to Task 1's run.

- [ ] **Step 4: Confirm fmt and clippy are still clean**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both exit 0.

- [ ] **Step 5: Commit**

```bash
git add crates/mesh-semantic/src/lib.rs
git commit -m "docs(mesh-semantic): document every public IR type and lower()"
```

---

### Task 3: mesh-parser — explicit match arms, ParseError Display/Error, docs

**Files:**
- Modify: `crates/mesh-parser/src/lib.rs`
- Modify: `crates/mesh-parser/Cargo.toml`
- Test: `crates/mesh-parser/tests/parse.rs`

**Interfaces:**
- Consumes: nothing new from earlier tasks.
- Produces: `ParseError` now implements `std::fmt::Display` (via `#[error("{message}")]`) and `std::error::Error` — later tasks (Task 5, mesh-cli) don't need to change how they consume `ParseError`, since `mesh_compiler::Diagnostic` already copies `err.message`/`err.span` by value rather than holding the `ParseError` itself.

- [ ] **Step 1: Add the failing test for Display/Error**

Add to the end of `crates/mesh-parser/tests/parse.rs` (the file currently ends after `decodes_string_escape_sequences`, at line 229 — append after it, no blank-line changes needed elsewhere in the file):

```rust

#[test]
fn parse_error_implements_display_and_std_error() {
    let error = mesh_parser::parse("<page").expect_err("should fail to parse");

    // Display uses the message.
    assert_eq!(error.to_string(), "syntax error");

    // Implements std::error::Error — this is a compile-time check: if
    // ParseError didn't implement the trait, this generic call wouldn't
    // type-check.
    fn assert_is_std_error<E: std::error::Error>(_: &E) {}
    assert_is_std_error(&error);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p mesh-parser parse_error_implements_display_and_std_error 2>&1 | tail -30`
Expected: FAIL — compile error, `ParseError` doesn't implement `Display` (`error.to_string()` won't resolve) or `std::error::Error`.

- [ ] **Step 3: Add the `thiserror` dependency**

In `crates/mesh-parser/Cargo.toml`, change:

```toml
[dependencies]
mesh-syntax.workspace = true
tree-sitter = "0.27.0"
tree-sitter-mprx = { version = "0.1.0", path = "../../grammar/tree-sitter-mprx" }
```

to:

```toml
[dependencies]
mesh-syntax.workspace = true
tree-sitter = "0.27.0"
tree-sitter-mprx = { version = "0.1.0", path = "../../grammar/tree-sitter-mprx" }
thiserror = "2"
```

- [ ] **Step 4: Derive Display/Error on ParseError, fix the wildcard match arms, add doc comments**

In `crates/mesh-parser/src/lib.rs`:

Replace lines 1–19 (the crate doc comment through the `parse` function signature) with:

```rust
//! Parses MPRX source text via the `tree-sitter-mprx` grammar and lowers the
//! resulting concrete syntax tree into the `mesh-syntax` AST.
//!
//! The Tree-sitter CST is an implementation detail of this crate — it is
//! never exposed as the application's final UI model.

use mesh_syntax::{
    Attribute, AttributeValue, BooleanLiteral, Child, Element, Expression, Literal, MemberAccess,
    NullLiteral, NumberLiteral, Reference, Span, StringLiteral, Text,
};
use tree_sitter::Node;

/// An error produced while parsing MPRX source text.
///
/// Currently carries a single message and the source [`Span`] it applies
/// to. `mesh_parser::parse`/`mesh_semantic::lower`'s signatures will need
/// to change to carry multiple diagnostics once Pass 4 needs more than one
/// per compile — see the v0.1 roadmap.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// Parses `source` as MPRX and lowers the resulting concrete syntax tree
/// into the [`mesh_syntax`] AST.
///
/// # Errors
///
/// Returns [`ParseError`] if the source fails to parse outright, if the
/// resulting tree contains a syntax error, or if the tree does not contain
/// exactly one root element.
pub fn parse(source: &str) -> Result<Element, ParseError> {
```

(Lines 20 onward — the body of `parse` — are unchanged; only the doc comment and the derive/doc on `ParseError` are new.)

Then fix the four wildcard match arms. Replace lines 83–92 (`fn lower_child`):

```rust
fn lower_child(node: Node, source: &str) -> Child {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "expression_block" => Child::Expression(lower_expression_block(inner, source)),
        _ => Child::Text(Text {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
    }
}
```

with:

```rust
fn lower_child(node: Node, source: &str) -> Child {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "expression_block" => Child::Expression(lower_expression_block(inner, source)),
        "text" => Child::Text(Text {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
        other => unreachable!(
            "unexpected child node kind {other:?} inside a successfully-parsed tree"
        ),
    }
}
```

Replace lines 164–172 (`fn lower_expression`):

```rust
fn lower_expression(node: Node, source: &str) -> Expression {
    // `node` is an `expression` node; its single child is the real variant.
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "literal" => Expression::Literal(lower_literal(inner, source)),
        "member_access" => Expression::MemberAccess(lower_member_access(inner, source)),
        _ => Expression::Reference(lower_reference(inner, source)),
    }
}
```

with:

```rust
fn lower_expression(node: Node, source: &str) -> Expression {
    // `node` is an `expression` node; its single child is the real variant.
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "literal" => Expression::Literal(lower_literal(inner, source)),
        "member_access" => Expression::MemberAccess(lower_member_access(inner, source)),
        "reference" => Expression::Reference(lower_reference(inner, source)),
        other => unreachable!(
            "unexpected expression node kind {other:?} inside a successfully-parsed tree"
        ),
    }
}
```

Replace lines 174–190 (`fn lower_literal`):

```rust
fn lower_literal(node: Node, source: &str) -> Literal {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "string" => Literal::String(lower_string(inner, source)),
        "boolean_literal" => Literal::Boolean(BooleanLiteral {
            value: text_of(inner, source) == "true",
            span: span_of(inner),
        }),
        "null_literal" => Literal::Null(NullLiteral {
            span: span_of(inner),
        }),
        _ => Literal::Number(NumberLiteral {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
    }
}
```

with:

```rust
fn lower_literal(node: Node, source: &str) -> Literal {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "string" => Literal::String(lower_string(inner, source)),
        "boolean_literal" => Literal::Boolean(BooleanLiteral {
            value: text_of(inner, source) == "true",
            span: span_of(inner),
        }),
        "null_literal" => Literal::Null(NullLiteral {
            span: span_of(inner),
        }),
        "number_literal" => Literal::Number(NumberLiteral {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
        other => unreachable!(
            "unexpected literal node kind {other:?} inside a successfully-parsed tree"
        ),
    }
}
```

Replace lines 199–218 (`fn lower_member_access`):

```rust
fn lower_member_access(node: Node, source: &str) -> MemberAccess {
    let object = node
        .child_by_field_name("object")
        .map(|o| match o.kind() {
            "member_access" => Expression::MemberAccess(lower_member_access(o, source)),
            _ => Expression::Reference(lower_reference(o, source)),
        })
        .unwrap_or_else(|| Expression::Reference(lower_reference(node, source)));

    let property = node
        .child_by_field_name("property")
        .map(|p| text_of(p, source))
        .unwrap_or_default();

    MemberAccess {
        object: Box::new(object),
        property,
        span: span_of(node),
    }
}
```

with:

```rust
fn lower_member_access(node: Node, source: &str) -> MemberAccess {
    let object = node
        .child_by_field_name("object")
        .map(|o| match o.kind() {
            "member_access" => Expression::MemberAccess(lower_member_access(o, source)),
            "reference" => Expression::Reference(lower_reference(o, source)),
            other => unreachable!(
                "unexpected member-access object kind {other:?} inside a successfully-parsed tree"
            ),
        })
        .unwrap_or_else(|| Expression::Reference(lower_reference(node, source)));

    let property = node
        .child_by_field_name("property")
        .map(|p| text_of(p, source))
        .unwrap_or_default();

    MemberAccess {
        object: Box::new(object),
        property,
        span: span_of(node),
    }
}
```

Everything else in the file (`lower_element`, `lower_attribute`, `lower_expression_block`, `lower_string`, `decode_string_escapes`, `lower_reference`, `text_of`, `span_of`) is unchanged.

- [ ] **Step 5: Run the new test to confirm it passes**

Run: `cargo test -p mesh-parser parse_error_implements_display_and_std_error 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Run the full test suite to confirm zero behavior change from the match-arm fix**

The wildcard-to-explicit-arm change is a refactor, not new behavior — every currently-known node kind was already correctly classified by the old wildcard (there's only ever been one implicitly-caught kind per match site). The proof that nothing changed is that every existing test in `parse.rs` (which exercises `"reference"`, `"number_literal"`, and `"text"` nodes through the code paths just changed) still passes identically.

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: PASS — the new `parse_error_implements_display_and_std_error` test plus every pre-existing test, all passing.

- [ ] **Step 7: Confirm fmt and clippy are still clean**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both exit 0. (Clippy may have an opinion on `unreachable!()` formatting — if it does, apply its suggestion; it will not ask you to remove the `unreachable!()` itself.)

- [ ] **Step 8: Commit**

```bash
git add crates/mesh-parser/src/lib.rs crates/mesh-parser/Cargo.toml crates/mesh-parser/tests/parse.rs Cargo.lock
git commit -m "fix(mesh-parser): replace silent wildcard fallbacks with explicit matches; add ParseError Display/Error"
```

---

### Task 4: mesh-compiler — Severity non_exhaustive, Diagnostic Display, docs

**Files:**
- Modify: `crates/mesh-compiler/src/lib.rs`
- Test: `crates/mesh-compiler/tests/compile.rs`

**Interfaces:**
- Consumes: nothing new from earlier tasks (mesh-compiler already depends on mesh-parser's `ParseError`, but only reads `.message`/`.span` by value — Task 3's Display/Error addition to `ParseError` doesn't change that).
- Produces: `Diagnostic` now implements `std::fmt::Display` (format: `"{message} ({start}..{end})"`) — Task 5 (mesh-cli) consumes this to simplify its own formatting. `Severity` is now `#[non_exhaustive]` — Task 5 doesn't match on `Severity` today and doesn't need to change.

- [ ] **Step 1: Add the failing test for Diagnostic's Display**

Add to the end of `crates/mesh-compiler/tests/compile.rs` (the file currently ends after `compiling_invalid_source_produces_an_error_diagnostic` — append after it):

```rust

#[test]
fn diagnostic_display_includes_message_and_span() {
    let result = mesh_compiler::compile("<page");
    let diagnostic = &result.diagnostics[0];

    assert_eq!(diagnostic.to_string(), "syntax error (0..5)");
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p mesh-compiler diagnostic_display_includes_message_and_span 2>&1 | tail -30`
Expected: FAIL — compile error, `Diagnostic` doesn't implement `Display` (`.to_string()` won't resolve).

- [ ] **Step 3: Implement Display, `#[non_exhaustive]`, and doc comments**

Replace the full contents of `crates/mesh-compiler/src/lib.rs` with:

```rust
//! Type checking, diagnostics, transformation, optimization, and code
//! generation for MPRX, built on the semantic model in `mesh-semantic`.
//!
//! For v0.1 this crate only orchestrates parse → lower and aggregates
//! diagnostics; type checking against a component model is not yet
//! implemented (see the v0.1 roadmap design spec).

use std::fmt;

/// How serious a [`Diagnostic`] is.
///
/// Only `Error` exists for v0.1 — a compile either fully succeeds or
/// produces exactly one fatal error. Marked `#[non_exhaustive]` because
/// the Pass 4 diagnostics work is expected to add more variants (e.g.
/// `Warning`), and that should not be a breaking change for consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

/// A single compile-time diagnostic: a message, its severity, and the
/// source [`mesh_syntax::Span`] it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: mesh_syntax::Span,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}..{})",
            self.message, self.span.start_byte, self.span.end_byte
        )
    }
}

/// The result of compiling one MPRX source file: the Semantic IR, if
/// compilation succeeded, and every diagnostic produced along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Compiles `source`: parses it, lowers the AST into the Semantic IR, and
/// collects any diagnostics produced along the way.
///
/// Returns a [`CompileResult`] rather than a `Result` because a compile
/// will be able to produce diagnostics without failing outright once
/// warnings exist (Pass 4) — check `diagnostics.is_empty()` or
/// `ir.is_some()` depending on what you need.
pub fn compile(source: &str) -> CompileResult {
    match mesh_parser::parse(source) {
        Ok(ast) => {
            let ir = mesh_semantic::lower(&ast);
            CompileResult {
                ir: Some(ir),
                diagnostics: vec![],
            }
        }
        Err(err) => CompileResult {
            ir: None,
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                message: err.message,
                span: err.span,
            }],
        },
    }
}
```

The `compile` function body is byte-for-byte unchanged; everything new is the `use std::fmt;` import, the doc comments, `#[non_exhaustive]`, and the `impl fmt::Display for Diagnostic` block.

- [ ] **Step 4: Run the new test to confirm it passes**

Run: `cargo test -p mesh-compiler diagnostic_display_includes_message_and_span 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: PASS — in particular `compiling_invalid_source_produces_an_error_diagnostic` (which does `assert_eq!(result.diagnostics[0].severity, mesh_compiler::Severity::Error)`) still passes: `#[non_exhaustive]` only restricts exhaustive *matching* and struct-literal construction of *new* variants from other crates, not equality comparison or referencing an existing unit variant.

- [ ] **Step 6: Confirm fmt and clippy are still clean**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/mesh-compiler/src/lib.rs crates/mesh-compiler/tests/compile.rs
git commit -m "feat(mesh-compiler): implement Display for Diagnostic, mark Severity non_exhaustive, add docs"
```

---

### Task 5: mesh-cli — use Diagnostic's Display, clap help docs, tempfile fixture cleanup

**Files:**
- Modify: `crates/mesh-cli/src/main.rs`
- Modify: `crates/mesh-cli/Cargo.toml`
- Modify: `crates/mesh-cli/tests/check.rs`

**Interfaces:**
- Consumes: `Diagnostic`'s `impl fmt::Display` from Task 4.
- Produces: nothing later tasks depend on — mesh-cli is the top of the dependency graph.

- [ ] **Step 1: Simplify main.rs to use Diagnostic's Display, add clap help docs**

Replace the full contents of `crates/mesh-cli/src/main.rs` with:

```rust
//! Native CLI entry point for the MESH toolchain.

use clap::{Parser, Subcommand};
use std::fs;
use std::process::ExitCode;

/// The MESH command-line toolchain: parse, check, and (eventually) compile
/// MPRX source files.
#[derive(Parser)]
#[command(name = "mesh")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate an MPRX file, printing any diagnostics.
    Check {
        /// Path to the `.mprx` file to check.
        file: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => run_check(&file),
    }
}

fn run_check(file: &str) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: could not read {file}: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = mesh_compiler::compile(&source);

    if result.diagnostics.is_empty() {
        println!("no errors");
        return ExitCode::SUCCESS;
    }

    for diagnostic in &result.diagnostics {
        eprintln!("error: {diagnostic}");
    }
    ExitCode::FAILURE
}
```

Changes from the current file: `Command::Check`'s `file` field now has a `///` doc comment (shown by clap in `--help`), `Cli` has a `///` doc comment, and the `eprintln!` loop now uses `Diagnostic`'s `Display` impl (`"error: {diagnostic}"`) instead of manually formatting `.message`/`.span.start_byte`/`.span.end_byte` — same output shape, since `Diagnostic`'s `Display` produces exactly `"{message} ({start}..{end})"`, matching what the old manual format string produced.

- [ ] **Step 2: Add the `tempfile` dev-dependency**

In `crates/mesh-cli/Cargo.toml`, change:

```toml
[dev-dependencies]
assert_cmd = "2.2.2"
predicates = "3.1.4"
```

to:

```toml
[dev-dependencies]
assert_cmd = "2.2.2"
predicates = "3.1.4"
tempfile = "3"
```

- [ ] **Step 3: Fix the leaking test fixture**

Replace the full contents of `crates/mesh-cli/tests/check.rs` with:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

#[test]
fn check_reports_no_errors_for_the_canonical_pass_1_example() {
    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("../../examples/page.mprx")
        .assert()
        .success()
        .stdout(predicate::str::contains("no errors"));
}

#[test]
fn check_reports_an_error_for_invalid_source() {
    let mut file = tempfile::Builder::new()
        .suffix(".mprx")
        .tempfile()
        .expect("should create a temp file");
    write!(file, "<page").expect("should write to the temp file");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}
```

`file` is a `tempfile::NamedTempFile`; it deletes itself when dropped at the end of the test, and its path is unique per test run (no more fixed filename, no more manual `std::env::temp_dir()` bookkeeping).

- [ ] **Step 4: Run mesh-cli's tests to confirm they pass**

Run: `cargo test -p mesh-cli 2>&1 | tail -30`
Expected: PASS — both `check_reports_no_errors_for_the_canonical_pass_1_example` and `check_reports_an_error_for_invalid_source`.

- [ ] **Step 5: Confirm the temp file is actually gone after the test run**

Run: `ls "$(dirname "$(mktemp -u)")"/*.mprx 2>&1`
Expected: `No such file or directory` (or equivalent "no match") — no leftover `.mprx` file in the OS temp directory from this or any previous run of this test.

- [ ] **Step 6: Run the full test suite**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: PASS — every test in the workspace, including Tasks 1–4's additions.

- [ ] **Step 7: Confirm fmt and clippy are still clean**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/mesh-cli/src/main.rs crates/mesh-cli/Cargo.toml crates/mesh-cli/tests/check.rs Cargo.lock
git commit -m "fix(mesh-cli): stop leaking a test fixture into the OS temp dir; use Diagnostic's Display; add clap help docs"
```

---

### Task 6: Centralize lint policy with `[workspace.lints]`

**Files:**
- Modify: `Cargo.toml` (root)
- Modify: `crates/mesh-syntax/Cargo.toml`
- Modify: `crates/mesh-parser/Cargo.toml`
- Modify: `crates/mesh-semantic/Cargo.toml`
- Modify: `crates/mesh-compiler/Cargo.toml`
- Modify: `crates/mesh-cli/Cargo.toml`
- Modify: `crates/mesh-lsp/Cargo.toml`

**Interfaces:**
- Consumes: nothing — this is a build-configuration change, runs last specifically so it validates every source change Tasks 1–5 made (new `thiserror`/`tempfile` deps, new `unreachable!()` calls, new `#[non_exhaustive]`, new doc comments) under centralized strict lints, not just under CI's `-D warnings` flag.
- Produces: nothing later tasks depend on (this is the last task).

Deliberately not touching `grammar/tree-sitter-mprx/Cargo.toml` — per Global Constraints, that crate is generated/vendored tree-sitter bindings, not hand-written project code, and already has its own targeted `#[allow(clippy::const_is_empty)]` for a generator quirk that centralizing lints shouldn't disturb.

- [ ] **Step 1: Prove the current CI-flag-only enforcement has a real gap**

Temporarily add an obviously-unused variable to `crates/mesh-syntax/src/lib.rs` — insert this as the very first line *inside* the `tests` module's `constructs_a_self_closing_element` function body (right after the `fn constructs_a_self_closing_element() {` line):

```rust
        let scratch_unused_check = 1;
```

Run: `cargo clippy --workspace --all-targets 2>&1 | tail -15`
Expected: clippy prints a warning about the unused variable (or an `unused variables` rustc lint) but **exits 0** — proving that without CI's `-D warnings` flag, a real warning doesn't fail the build today.

Remove the line you just added (`git diff crates/mesh-syntax/src/lib.rs` should show it, then revert it — do not commit this scratch change):

```bash
git checkout -- crates/mesh-syntax/src/lib.rs
```

- [ ] **Step 2: Add the workspace lint tables**

In the root `Cargo.toml`, add this section after `[workspace.dependencies]` (at the end of the file):

```toml

[workspace.lints.rust]
warnings = "deny"

[workspace.lints.clippy]
all = "deny"
```

- [ ] **Step 3: Opt every hand-written crate into the workspace lints**

In each of these six files, add a `[lints]` table (place it directly after the `[package]` table, before `[dependencies]`):

```toml
[lints]
workspace = true
```

Files: `crates/mesh-syntax/Cargo.toml`, `crates/mesh-parser/Cargo.toml`, `crates/mesh-semantic/Cargo.toml`, `crates/mesh-compiler/Cargo.toml`, `crates/mesh-cli/Cargo.toml`, `crates/mesh-lsp/Cargo.toml`.

- [ ] **Step 4: Re-run the same scratch check from Step 1 — this time it must fail without any CLI flag**

Re-add the same line to the same spot in `crates/mesh-syntax/src/lib.rs`:

```rust
        let scratch_unused_check = 1;
```

Run: `cargo clippy --workspace --all-targets 2>&1 | tail -15`
Expected: clippy now **exits non-zero** (a real build failure) on the unused variable, with no `-D warnings` flag needed — proving the workspace lint table is what's enforcing it now, not just CI's invocation.

Remove the scratch line again:

```bash
git checkout -- crates/mesh-syntax/src/lib.rs
```

- [ ] **Step 5: Confirm the real (non-scratch) workspace is clean under the new table**

Run: `cargo clippy --workspace --all-targets 2>&1 | tail -30`
Expected: exit 0, no warnings — everything Tasks 1–5 added (thiserror's derive, `unreachable!()`, `#[non_exhaustive]`, the new doc comments) is clean under `clippy::all` deny.

- [ ] **Step 6: Run the full test suite and fmt check**

Run: `cargo test --workspace 2>&1 | tail -40 && cargo fmt --all -- --check`
Expected: tests PASS (identical results to Task 5's run — this task touches no source logic, only `Cargo.toml` files), fmt check exits 0.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/mesh-syntax/Cargo.toml crates/mesh-parser/Cargo.toml crates/mesh-semantic/Cargo.toml crates/mesh-compiler/Cargo.toml crates/mesh-cli/Cargo.toml crates/mesh-lsp/Cargo.toml
git commit -m "chore: centralize lint policy in [workspace.lints] instead of CI-only flags"
```

---

## Definition of Done

- [ ] All 6 findings from the audit are fixed: explicit match arms (finding 1), doc comments on every public item across all 5 hand-written crates (finding 2), `ParseError`/`Diagnostic` Display+Error (finding 3), `[workspace.lints]` centralization (finding 4), `Severity` is `#[non_exhaustive]` (finding 5), the mesh-cli test fixture uses `tempfile` (finding 6).
- [ ] `cargo test --workspace` passes in full.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets` passes with **no flag needed** (Task 6 proved this is now enforced by configuration, not just CI's `-D warnings`).
- [ ] No public function/type name or signature changed from what existed before this plan — only additive (doc comments, trait impls, `#[non_exhaustive]`) and internal (match-arm) changes.
- [ ] Finding 7 (property-based testing) remains explicitly deferred, not attempted by this plan.
