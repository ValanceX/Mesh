# Embedding the compiler (Rust)

The `mesh` CLI is a thin wrapper around a library API. This guide shows how to call that API from your own Rust code: compile MPRX, handle diagnostics, and walk the resulting **Semantic IR**.

## Adding the dependency

The v0.1 crates aren't on crates.io yet. Depend on them from git:

```toml
[dependencies]
mesh-compiler = { git = "https://github.com/ValanceX/Mesh", tag = "v0.1.0" }
mesh-syntax   = { git = "https://github.com/ValanceX/Mesh", tag = "v0.1.0" }
mesh-semantic = { git = "https://github.com/ValanceX/Mesh", tag = "v0.1.0" }
```

The parser builds a Tree-sitter grammar written in C, so your build machine needs a C compiler. Rust's standard setup already includes one on most platforms.

The crates you'll use:

| Crate | What you get from it |
|---|---|
| `mesh-compiler` | `compile`, `CompileResult`, `render_diagnostic` |
| `mesh-syntax` | `Diagnostic`, `DiagnosticCode`, `Severity`, `Span` |
| `mesh-semantic` | The Semantic IR types: `Element`, `Attribute`, `EventBinding`, `Child`, `Expression`, ... |
| `mesh-manifest` | `load`, `Manifest`, `Template`, and the manifest's model types |

## Compiling a source string

```rust
use mesh_compiler::{compile, render_diagnostic};
use mesh_syntax::Severity;

fn main() {
    let path = "user-card.mprx";
    let source = r#"<user-card user={user} label="Hi" label="Hello" />"#;

    let result = compile(source);

    for diagnostic in &result.diagnostics {
        eprintln!("{}\n", render_diagnostic(source, path, diagnostic));
    }

    let has_errors = result
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error);
    if has_errors {
        std::process::exit(1);
    }

    println!("{:#?}", result.ir);
}
```

`compile` never panics and never returns `Err`. It always returns a `CompileResult`:

```rust
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<mesh_syntax::Diagnostic>,
}
```

- **`ir`** is `None` only when the source couldn't be parsed, which means there's a syntax error. Validation problems like a mismatched closing tag or duplicate attributes are reported as diagnostics, but `ir` is still `Some`, so tools can keep working with a file that has mistakes.
- **`diagnostics`** lists everything found, in a stable order (see the [diagnostics reference](../manual/diagnostics.md#ordering)). Decide what counts as failure by checking `severity`, as above. Don't use `diagnostics.is_empty()` for that, because warnings are allowed.

To compile a template against a component manifest, load the manifest with `mesh_manifest::load`, pick the component with `Manifest::template`, and pass it through `CompileOptions`:

```rust
use mesh_compiler::{compile_with, render_diagnostic, CompileOptions};

fn check_template(manifest_path: &str, manifest_json: &str, source: &str) {
    let manifest = match mesh_manifest::load(manifest_json) {
        Ok(manifest) => manifest,
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("{}\n", render_diagnostic(manifest_json, manifest_path, diagnostic));
            }
            return;
        }
    };
    let template = match manifest.template("users-page") {
        Ok(template) => template,
        Err(diagnostic) => {
            eprintln!("{}\n", render_diagnostic(manifest_json, manifest_path, &diagnostic));
            return;
        }
    };
    let result = compile_with(source, &CompileOptions::with_template(template));
    for diagnostic in &result.diagnostics {
        eprintln!("{}\n", render_diagnostic(source, "users-page.mprx", diagnostic));
    }
}
```

A manifest diagnostic's span indexes the manifest's text, not the `.mprx` source, so render it against the manifest, as above. In this version the template isn't used yet: `compile_with` returns exactly what `compile` does.

## Diagnostics

```rust
pub struct Diagnostic {
    pub severity: Severity,   // Severity::Error or Severity::Warning
    pub code: DiagnosticCode, // stable, e.g. DiagnosticCode::MISMATCHED_CLOSING_TAG
    pub message: String,
    pub span: Span,           // byte offsets into the source
}

pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}
```

- `Severity` is `#[non_exhaustive]`. Match it with a wildcard arm, because later versions may add levels.
- It implements `Display` as `error` / `warning`.
- `DiagnosticCode` is the diagnostic's stable code. Compare it with the associated constants, such as `DiagnosticCode::SYNTAX_ERROR`, or get the kebab-case string with `as_str()` or `{}`. `DiagnosticCode::ALL` lists every code. A code is never renamed or reused for a different meaning, so it's safe to match on; messages may change. The [diagnostics reference](../manual/diagnostics.md) documents each one.
- `Diagnostic` is plain data. Printing it with `{}` gives a compact `message (start..end)` form, with no severity, no code and no snippet.

For the same output the CLI prints, use `render_diagnostic`:

```rust
pub fn render_diagnostic(source: &str, path: &str, diagnostic: &Diagnostic) -> String
```

- It returns one block with **no trailing newline**, so you control the spacing between blocks.
- `path` is only displayed. The function doesn't read the file, so you can pass any label, such as a URL or `"<generated>"`.
- It never panics, even when a span is out of range or falls inside a multi-byte character. Such spans are clamped.

The block format is documented in the [CLI manual](../manual/mesh-cli.md#diagnostic-format).

## Walking the Semantic IR

The IR is a plain tree of owned values. Every node records where it came from in the source. For `examples/user-card.mprx`:

```xml
<user-card
  user={user}
  compact={layout.compact}
  on.select={selectUser($event)} />
```

`compile` produces:

```text
Element {
    name: "user-card",
    attributes: [
        Attribute { name: "user",    value: Expression(Reference { name: "user" }) },
        Attribute { name: "compact", value: Expression(MemberAccess { object: Reference { name: "layout" }, property: "compact" }) },
    ],
    event_bindings: [
        EventBinding {
            name: "select",
            handler: Command { command: "selectUser", arguments: [EventValue { name: "event" }] },
        },
    ],
    children: [],
}
```

(Spans are left out above; every node has them. See [Source spans](#source-spans).)

The main types, all from `mesh_semantic`:

```rust
pub struct Element {
    pub name: String,
    pub name_span: Span,                     // just the tag name
    pub attributes: Vec<Attribute>,
    pub event_bindings: Vec<EventBinding>,   // name has no "on." prefix: "select"
    pub children: Vec<Child>,
    pub span: Span,                          // the whole element
}

pub struct Attribute { pub name: String, pub name_span: Span, pub value: AttributeValue, pub span: Span }
pub struct EventBinding { pub name: String, pub name_span: Span, pub handler: Expression, pub span: Span }

pub enum AttributeValue { String { value: String, span: Span }, Expression(Expression) }

pub enum Child {
    Text { value: String, span: Span },  // whitespace-only text is already removed
    Expression(Expression),
    Element(Box<Element>),
}

pub enum Expression {
    Literal { value: Literal, span: Span },  // String, Number (kept as source text), Boolean, Null
    Reference { name: String, span: Span },
    MemberAccess { object: Box<Expression>, property: String, property_span: Span, span: Span },
    Unary { operator: UnaryOperator, operand: Box<Expression>, span: Span },
    Binary { operator: BinaryOperator, left: Box<Expression>, right: Box<Expression>, span: Span },
    Conditional { condition: Box<Expression>, consequent: Box<Expression>, alternate: Box<Expression>, span: Span },
    Array { elements: Vec<Expression>, span: Span },
    Object { members: Vec<ObjectMember>, span: Span }, // ObjectMember { key, key_span, value, span }
    Command { command: String, command_span: Span, arguments: Vec<Expression>, span: Span },
    EventValue { name: String, span: Span },            // "$event" is stored as "event"
}
```

Some guarantees:

- **Duplicates are already resolved.** Each attribute and event name appears at most once per element, and the last occurrence in the source is the one kept.
- **String escapes are decoded.** `"a\"b"` becomes `a"b`.
- **Numbers stay as text** (`Literal::Number("3.5")`), so you choose how to parse them. Negative numbers are `Unary { operator: Negate, .. }` around a number.
- **`UnaryOperator` and `BinaryOperator`** are the `mesh_syntax` enums, reused as-is.

A small recursive walk:

```rust
use mesh_semantic::{AttributeValue, Child, Element};

fn print_tree(element: &Element, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{indent}<{}>", element.name);
    for attribute in &element.attributes {
        match &attribute.value {
            AttributeValue::String { value, .. } => println!("{indent}  {} = {value:?}", attribute.name),
            AttributeValue::Expression(expr) => println!("{indent}  {} = {expr:?}", attribute.name),
        }
    }
    for binding in &element.event_bindings {
        println!("{indent}  on.{} -> {:?}", binding.name, binding.handler);
    }
    for child in &element.children {
        match child {
            Child::Text { value, .. } => println!("{indent}  text {value:?}"),
            Child::Expression(expr) => println!("{indent}  expr {expr:?}"),
            Child::Element(child) => print_tree(child, depth + 1),
        }
    }
}
```

### Source spans

Every IR node has a `span`: a `mesh_syntax::Span { start_byte, end_byte }` covering the construct in the source. Where a part of a construct can be wrong on its own, it has its own span too:

| Field | Covers |
|---|---|
| `Element.name_span` | the opening tag's name: `user-card` |
| `Attribute.name_span` | the attribute's name, without `=` or the value |
| `EventBinding.name_span` | the event name after `on.`: `select` |
| `MemberAccess.property_span` | the property after the `.` |
| `Command.command_span` | the command's name, without the arguments |
| `ObjectMember.key_span` | the key as written, quotes included for a quoted key |

`Expression::span()` and `AttributeValue::span()` return the span of any variant.

- Offsets are **bytes** into the source exactly as you passed it, including a leading byte-order mark. They are always on character boundaries, so `&source[span.start_byte..span.end_byte]` is the construct's text.
- A string attribute value's span includes its quotes. A parenthesized expression's span doesn't include the parentheses; `(a + b)` has no node of its own.
- To show a span as a line and column, use `render_diagnostic`, or count lines yourself.
- Spans make IR values from different sources compare unequal even when they mean the same thing. To compare meaning, zero the spans first.

## Stability

v0.1 is the first release. The IR's *meaning* is intended to be stable, but its Rust types may still change before 1.0: v0.2, for example, adds source spans, which changes several variants' shapes. In particular, v0.1 has no serialized IR format, so treat the IR as an in-process value. Pin a tag rather than tracking `main`.
