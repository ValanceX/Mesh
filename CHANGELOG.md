# Changelog

All notable changes to MESH are recorded here. The project follows [Semantic Versioning](https://semver.org). Until 1.0, minor versions may include breaking changes.

## [Unreleased]

### Added

- **`mesh --version`** (and `-V`) prints the installed version.
- **Diagnostic codes.** Every diagnostic has a stable, kebab-case code, shown in brackets after its severity: `error[mismatched-closing-tag]: …`. A code is never renamed or reused for a different meaning. The [diagnostics reference](./docs/manual/diagnostics.md) lists every code. In the Rust API, `Diagnostic` and `ParseError` have a new `code: DiagnosticCode` field.
- **Located syntax errors.** Most syntax errors are now reported where the mistake is, instead of at `1:1`, and a file with several syntax errors reports each one. Common mistakes get their own code and message: `unterminated-tag`, `missing-closing-tag`, `less-than-in-text`, `hyphenated-attribute-name`, `single-brace-object`, `command-trailing-comma` and `malformed-event-binding`. Anything else keeps the `syntax-error` code.

### Changed

- **Toolchain.** The repo pins its Rust toolchain in `rust-toolchain.toml`, and the minimum supported Rust version is 1.91. CI checks both.
- **Diagnostic header.** The first line of a diagnostic is now `<severity>[<code>]: <message>` instead of `<severity>: <message>`. Which files pass or fail, and the exit statuses, are unchanged.
- **Rust API.** Code that builds a `Diagnostic` or `ParseError` with a struct literal must set the new `code` field.

### Fixed

- **Byte-order marks.** A file that starts with a UTF-8 byte-order mark no longer reports line 1's columns one position too far right, and the invisible mark is no longer echoed in the source snippet.

## [0.1.0] - 2026-09-24

The first release. See the [v0.1 release notes](./docs/releases/v0.1.md) for an overview.

### Added

- **Grammar and parser.** A Tree-sitter grammar for MPRX, plus `mesh-parser`, which lowers the concrete syntax tree into the `mesh-syntax` AST with byte-offset source spans.
- **Elements and text.** Elements with self-closing and container forms, hyphenated tag names, string attributes with `\"`, `\\`, `\n` and `\t` escapes, plain text children, and nested elements.
- **Expressions.** `{...}` expressions in attribute values and element content:
  - string, number, boolean and `null` literals
  - references and member access
  - unary `!` and `-`
  - binary arithmetic, comparison, equality and logical operators, with C-family precedence
  - conditional `? :`
  - arrays and objects (with trailing commas allowed)
  - command invocations
  - `$event`
- **Event bindings.** `on.<event>={expression}`.
- **Semantic IR.** `mesh-semantic`, a span-free Semantic IR. Whitespace-only text is dropped, and string escapes are decoded.
- **Structural validation.**
  - A mismatched closing tag is an error.
  - A duplicate attribute or duplicate event binding is a warning, and the last occurrence wins.
  - Validation diagnostics are non-fatal, so IR is still produced.
- **Compiler API.** `mesh-compiler` provides `compile(&str) -> CompileResult { ir, diagnostics }`. Diagnostics are aggregated across parsing and lowering, in a stable order.
- **Rendering.** `mesh_compiler::render_diagnostic` produces diagnostic blocks in the Rust compiler's style:
  - columns count characters
  - tabs are preserved
  - Windows `\r\n` line endings are handled
  - out-of-range spans are clamped, so rendering never panics
- **Display for Severity.** `mesh_syntax::Severity` implements `Display` (`error` / `warning`).
- **CLI.** `mesh check <FILE>`:
  - diagnostics go to stderr, and `no errors` goes to stdout
  - exits `1` only on errors or an unreadable file
  - warnings alone exit `0`
- **Examples.** The canonical examples `examples/user-card.mprx` and `examples/users-page.mprx`, and a fixture corpus in `examples/fixtures/{pass,fail}/`, each file paired with its exact expected output.
- **CI.** Checks formatting, runs clippy with warnings denied, and runs the full test suite.
- **Documentation.** The language spec, architecture overview, getting-started guide, language guide, CLI manual, diagnostics reference, and embedding guide.

### Known limitations

- Syntax errors are reported at `1:1`, spanning the whole document.
- No component-model or type checking yet.
- `mesh-lsp` and the npm packages are placeholders.

[0.1.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.1.0
