# Changelog

All notable changes to MESH are recorded here. The project follows [Semantic Versioning](https://semver.org). Until 1.0, minor versions may include breaking changes.

## [Unreleased]

### Fixed

- **Deeply nested files no longer crash MESH.** A valid `.mprx` file nested a few thousand levels deep overflowed the stack and aborted `mesh check`, with no diagnostic and a signal instead of an exit status; a manifest nested tens of thousands of levels deep did the same, and loading one took time quadratic in its depth. Now a file may nest at most 128 levels deep. Every element counts as a level, and so does every expression inside another one and every pair of parentheses. A deeper file is a `nesting-too-deep` error, reported like a syntax error. A manifest whose JSON arrays and objects nest more than 128 levels deep is a `manifest-nesting-too-deep` error. In the Rust API, the limits are `mesh_parser::MAX_NESTING_DEPTH` and `mesh_manifest::MAX_NESTING_DEPTH`, and at those limits the whole pipeline, from parsing to rendering, runs on a 2 MiB thread stack.

### Changed

- **Breaking, in principle: the nesting limits.** A file or manifest nested more than 128 levels deep, which v0.2 accepted if it was shallow enough not to crash, is now rejected. No handwritten UI or manifest comes near the limit; to stay under it, flatten the file, or write deeply nested manifest types as named types.

## [0.2.0] - 2026-09-24

MPRX is now checked against a component model. See the [v0.2 release notes](./docs/releases/v0.2.md) for an overview.

### Added

- **`mesh --version`** (and `-V`) prints the installed version.
- **Diagnostic codes.** Every diagnostic has a stable, kebab-case code, shown in brackets after its severity: `error[mismatched-closing-tag]: …`. A code is never renamed or reused for a different meaning. The [diagnostics reference](./docs/manual/diagnostics.md) lists every code. In the Rust API, `Diagnostic` and `ParseError` have a new `code: DiagnosticCode` field.
- **Located syntax errors.** Most syntax errors are now reported where the mistake is, instead of at `1:1`, and a file with several syntax errors reports each one. Common mistakes get their own code and message: `unterminated-tag`, `missing-closing-tag`, `less-than-in-text`, `hyphenated-attribute-name`, `single-brace-object`, `command-trailing-comma` and `malformed-event-binding`. Anything else keeps the `syntax-error` code.
- **Component manifests.** `mesh check --model <MANIFEST> [--component <NAME>] <FILE>` loads a JSON component manifest that declares components (props, events, commands and expression scope) and named types. The whole manifest is checked first: after its `"version"` is accepted, every problem is reported at its line and column with a `manifest-*` code; a broken manifest stops the check. The file is the template of the component named by `--component`, or by its file name. The format is published as a JSON Schema in `schemas/manifest-v1.schema.json`, and `examples/components.json` declares the examples' components.
- **Rust API.** A new crate, `mesh-manifest`, loads and validates manifests (`load`, `Manifest`, `Template`). `mesh_compiler::compile_with` takes `CompileOptions`, which can carry a template; `compile` is unchanged.
- **Checking against a manifest.** With `--model`, `mesh check` now checks the file as its component's template: every tag must be a declared component, every attribute one of its props (with every required prop supplied), every `on.` binding one of its events, every reference a name in the template's scope, and every command one of the template's commands, with the right number of arguments. Commands may appear only as the whole handler of an `on.` binding, and `$event` only in that command's arguments. Each mistake is its own error code: `unknown-component`, `unknown-prop`, `missing-required-prop`, `unknown-event`, `unknown-reference`, `unknown-command`, `command-arity-mismatch`, `command-outside-handler`, `handler-not-command`, `event-value-outside-handler`, `event-has-no-payload` and `unknown-special-value`. Without `--model`, nothing changes.
- **Expression types.** With `--model`, every expression is typed. Reading a member that doesn't exist is `unknown-member`, and reading one through a value that may be absent is `possibly-absent-access`. Operators and conditions get the types they need or it's a `type-mismatch`: `+` adds numbers only, and nothing is converted implicitly. `==` and `!=`, the branches of `? :` and the elements of an array need a common type, or it's `no-common-type`. An object literal that repeats a key is `duplicate-object-key`: as with attributes, the last occurrence counts, and each earlier one is reported. One mistake gets one diagnostic: an expression that has an error isn't checked any further.
- **Value checking.** With `--model`, every prop value, command argument and `$event` is checked against its declared type, and a mismatch is a `type-mismatch`. A value that may be absent fits only where absence is allowed, and `null` is a value, not absence. An object literal given to a record type is checked field by field: a key the record doesn't declare is `unknown-field`, and a required field that's left out is `missing-required-field`. An array literal given to a list type is checked element by element, and a conditional given a type branch by branch, so their parts need no common type there. All of these checks use one compatibility rule.
- **Suggestions.** A misspelled name gets a `= help: did you mean "user"?` line when the manifest declares a close match. In the Rust API, `Diagnostic` has a new `suggestions` field, and `Suggestion` holds a replacement and the span it replaces.
- **JSON diagnostics.** `mesh check --format json` prints every diagnostic as one JSON document on stdout, for programs such as a generator that repairs its own output: its severity, code, message, path, and span (as byte offsets and as lines and columns), plus any suggestions. It reports exactly what the default `--format human` does, with the same exit status. The format is described in the [CLI manual](./docs/manual/mesh-cli.md#json-output) and published as a JSON Schema in `schemas/diagnostics-v1.schema.json`. In the Rust API, `mesh_compiler::render_json` builds the same document.
- **Rust API.** A new crate, `mesh-analysis`, analyzes a template's Semantic IR against its manifest (`analyze`, `Analysis`), keeping what each name resolved to, each expression's type (`Ty`) and each problem found beside the IR, never in it. `is_assignable` and `join` are the type relations every check uses. `compile_with` uses it when given a template.

### Changed

- **Toolchain.** The repo pins its Rust toolchain in `rust-toolchain.toml`, and the minimum supported Rust version is 1.91. CI checks both.
- **Diagnostic header.** The first line of a diagnostic is now `<severity>[<code>]: <message>` instead of `<severity>: <message>`. Which files pass or fail, and the exit statuses, are unchanged.
- **Rust API.** Code that builds a `Diagnostic` or `ParseError` with a struct literal must set the new `code` field, and a `Diagnostic` the new `suggestions` field too.
- **Breaking: source spans in the Semantic IR.** Every IR node now records where it came from, as byte offsets into the source (`span`), and names that a diagnostic can point at on their own have their own span (`name_span`, `property_span`, `command_span`, `key_span`). Several IR variants changed shape to hold them: `Expression::Reference(name)` is now `Expression::Reference { name, span }`, and likewise `Literal`, `Array`, `Object`, `EventValue`, `AttributeValue::String` and `Child::Text`; the other variants gained fields. `Expression::span()` and `AttributeValue::span()` read any variant's span. The IR's meaning is unchanged. The AST gained the same name spans. See the [embedding guide](./docs/guides/embedding-the-compiler.md#source-spans).

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

[0.2.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.2.0
[0.1.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.1.0
