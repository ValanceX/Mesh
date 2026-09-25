# Changelog

All notable changes to MESH are recorded here. The project follows [Semantic Versioning](https://semver.org). Until 1.0, minor versions may include breaking changes.

## [Unreleased]

### Added

- **MPRX's evaluation is specified** (`docs/MPRX-SPEC.md` §9.7): values and absence, the one runtime type relation (*fits*), IEEE 754 numbers with a truncated `%`, strict equality, evaluation order, the runtime checks, and text. Number-to-text conversion is pinned by a normative table, `docs/tables/number-to-text.tsv`, computed from the rule by an exact reference generator. The runtime that implements it arrives in later v0.5 passes.
- **The template format is specified** (`schemas/template-v1.schema.json`, `docs/manual/templates.md`): one checked component with every name resolved and no values, its compatibility rule (format version and model fingerprint only), the fingerprint's exact byte layout, and a template's canonical digest. Nothing emits one yet.
- **Programs and their assembly rules are specified** (`docs/manual/templates.md`): a root and a set of templates; a component is a composite exactly when the program has its template; how a composite's props bind its scope; and seven rules (one template per component, the root's template, well-formed templates of one model, sound bindings, no cycles, no composite events, no composite children), with their codes and order.
- **The render tree and the command intent are specified** (`schemas/render-v1.schema.json`, `docs/manual/runtime.md`): what a renderer draws (primitive nodes, final prop values, text runs, handler identifiers and keys, and nothing else), what a host receives from dispatch, the host's dispatch lifecycle, and the encodings of keys, handler identifiers and program identity.
- **The boundary between the runtime and its host is specified** (§9.8): which values may cross, how absence, `-0`, NaN and unpaired surrogates are treated, open snapshots, exact records, and how JSON and JavaScript values map onto them.

## [0.4.0] - 2026-09-25

MESH outside Rust: the compiler in JavaScript, as `@valancex/mesh-compiler`, and `mesh-lsp` from npm, as `@valancex/mesh-lsp`. See the [v0.4 release notes](./docs/releases/v0.4.md) for an overview.

### Added

- **Rust API: `mesh_compiler::check`, the whole check `mesh check` does,** for any program that hosts MESH: `Model::load` loads a manifest and looks up a component (reporting every manifest error, or a missing component, against the manifest), `check::source` checks a source against it or without a model, and `check::run` does both, returning a `Report` that renders itself as the diagnostics document against the right text and path. The `mesh` CLI is now a host of it; its output hasn't changed.
- **UTF-16 positions in `mesh check --format json`.** Every position also gives `utf16`, the same place as `byte` counted in UTF-16 code units (what a JavaScript string indexes, so `source.slice(start.utf16, end.utf16)` is the span's text), and `utf16Column`, its column in UTF-16 code units. The document's `version` is still 1, as its promise allows, and the schema lists both.
- **Rust API: `SourceMap::utf16_offset`,** the UTF-16 offset of a byte.
- **The compiler builds for WebAssembly,** as `crates/mesh-wasm`: `cargo build -p mesh-wasm --target wasm32-unknown-unknown --profile wasm`. The module imports nothing, is about 150 KB gzipped, and runs the same check as `mesh check`. It's what `@valancex/mesh-compiler` will ship; its exports are internal.
- **`@valancex/mesh-compiler`: MESH in JavaScript** (see [Using MESH from JavaScript](./docs/guides/using-mesh-from-javascript.md)). `check({ source, path, model? })` returns the diagnostics document `mesh check --format json` prints for the same inputs, from the compiler itself in WebAssembly; the package adds no rule of its own and returns diagnostics only. It runs in Node 22 and 24, where it loads its own module, and in browsers after `init()` with the module's URL. Paths are identifiers, never read. `MeshVersionError` refuses a module of another version; `MeshInternalError` reports a failure of the compiler itself, after which the next check uses a fresh instance.
- **A release workflow** builds every release from its tag: all of CI on the tagged commit, each `mesh-lsp` binary on its own OS and architecture's runner, the WebAssembly module, then the npm packages, published with provenance, and a GitHub release with the binaries attached.
- **`@valancex/mesh-lsp`: install the language server with npm.** `npm install -g @valancex/mesh-lsp` installs the native `mesh-lsp` for Linux x64 or arm64, macOS x64 or arm64, or Windows x64, from one platform package each (`@valancex/mesh-lsp-linux-x64` and so on), and `mesh-lsp` starts it, passing arguments, streams, exit codes and signals through unchanged. The Linux binaries are static. The editor setup guide shows both installs, and `editors/neovim/verify.sh` checks an npm-installed server with `MESH_LSP`.

### Changed

- **The npm packages are now `@valancex/mesh-compiler` and `@valancex/mesh-lsp`,** under ValanceX, the project's name, and versioned with the Rust workspace. The `mesh-language` placeholder is gone. None of the old names was ever published.

### Fixed

- **`mesh-lsp` on Windows: verbatim and UNC paths.** A path such as `\\?\C:\work\a.mprx`, the form Windows' path canonicalization returns, became a URI no client could read (`file:////%3F/C:/...`); it is now `file:///C:/work/a.mprx`. A network share, `\\server\share\a.mprx`, is now `file://server/share/a.mprx`.

## [0.3.0] - 2026-09-24

MESH in the editor: the `mesh-lsp` language server, a client of the compiler, and syntax highlighting. See the [v0.3 release notes](./docs/releases/v0.3.md) for an overview.

### Added

- **The `mesh-lsp` language server.** `cargo install --path crates/mesh-lsp` installs it, and any editor with an LSP client can run it over stdio. It publishes exactly the diagnostics `mesh check` reports, for `.mprx` documents and for the manifest, and keeps them current as you type, and it offers each suggestion as a quick fix. Each document's component is named explicitly in its settings (`mesh.model` and `mesh.components`), never guessed from a file name. It reads them from `initializationOptions` and, when the client supports it, `workspace/configuration`. It checks documents whose `languageId` is `mprx`, and recognizes the manifest by its path, whatever its language; every other file is ignored. When the settings change, what each open file is gets decided again, and while the manifest is open its unsaved text is the model. See the [`mesh-lsp` manual](./docs/manual/mesh-lsp.md).
- **Hover and go to definition in `mesh-lsp`.** Hover shows the type of the expression under the cursor, and what a component, prop, event, scope name or command is declared as. Go to definition jumps from any of those names to its key in the manifest. Both answer from the same check as the diagnostics, and on a file with a syntax error, from the parts of it that parse. A request made right after typing waits for the new text to be checked, and a newer change answers it `ContentModified`.
- **An editor setup guide** ([`docs/guides/editor-setup.md`](./docs/guides/editor-setup.md)): installing `mesh-lsp` and the MPRX parser, and configuring an editor for both. Its Neovim 0.11 configuration is `editors/neovim/init.lua`, which `editors/neovim/verify.sh` checks in a real, headless Neovim: diagnostics, hover, definition, completion on broken text, highlighting and manifest edits.
- **Syntax highlighting queries.** `grammar/tree-sitter-mprx/queries/highlights.scm` colours MPRX in any editor that loads Tree-sitter queries, with nvim-treesitter's capture names: tags, attributes and event names, strings, numbers, booleans, `null`, `$event`, names and members, commands, operators and punctuation. The queries only colour text; nothing in MESH reads them. A test pins every capture on sample files, and fails when the grammar gains a node the queries don't account for.
- **Completion in `mesh-lsp`.** Tags after `<`; a component's props (required ones first) and `on.` events in its opening tag; commands at the start of an `on.` handler; scope names wherever another expression starts; and a record's fields after `.`. Every item comes from the manifest and is offered only where the compiler accepts it, and it works while the file has syntax errors.
- **Rust API: completion.** `mesh_parser::context_at` says what kind of place a byte offset is in, read from the tokens before it, and `mesh_compiler::editor::candidates` lists the names that may be written there, derived from the manifest. Both are editor-only. `mesh_analysis::members` is the one rule for which members a type has, which the checker now reads members through too, and `mesh_analysis::read_field` gives a field's type as read.
- **Rust API: editor recovery.** `mesh_compiler::editor::recover` keeps the parts of a file with syntax errors that parse, and analyzes them against a template, for hover and go-to-definition in an editor. Its `Recovery` answers `resolution_at` and `typed_at` and has no diagnostics: it is never the compiler's verdict, and `compile`, `compile_with` and `mesh check` don't use it. Beneath it, `mesh_parser::recover`, `mesh_semantic::lower_expression` and `mesh_analysis::analyze_expression` (one expression, analyzed as a value by the same walk as a template) are public.
- **Rust API: `mesh_compiler::SourceMap`.** It maps a byte offset to a 0-based line and column, and back, counting columns in `char`s, UTF-8 bytes or UTF-16 code units (`ColumnUnit`). Its rules are exactly the ones `mesh check`'s output follows: a leading byte-order mark and a line's `\r` aren't columns, and out-of-range offsets and positions clamp instead of panicking. The human and JSON renderers now use it, with unchanged output.
- **Rust API: offset queries on `mesh_analysis::Analysis`.** `resolution_at`, `typed_at` and `facts_at` answer what an editor asks: what name, typed expression or problem is at this byte? The innermost match wins, and a span includes both of its ends, so a cursor just after a name still finds it.
- **Rust API: declaration spans in the manifest.** `Manifest::span_of(Declaration)` gives where a component, prop, event, command, parameter, scope name, named type or named record type's field is declared: its key, quotes included (a parameter's `"name"` value). They're kept beside the model, so `Type` and `Field` values still compare by meaning alone.

### Fixed

- **A closed pipe no longer crashes `mesh check`.** When whatever reads its output stops early, as in `mesh check --format json big.mprx | head -c 100`, `mesh check` used to panic with exit status 101. It now stops printing and exits with the check's own status.
- **JSON rendering is linear.** `render_json` located every span by scanning the source from the start, so a file with many diagnostics took time proportional to their number times the file's size.
- **Deeply nested files no longer crash MESH.** A valid `.mprx` file nested a few thousand levels deep overflowed the stack and aborted `mesh check`, with no diagnostic and a signal instead of an exit status; a manifest nested tens of thousands of levels deep did the same, and loading one took time quadratic in its depth. Now a file may nest at most 128 levels deep. Every element counts as a level, and so does every expression inside another one and every pair of parentheses. A deeper file is a `nesting-too-deep` error, reported like a syntax error. A manifest whose JSON arrays and objects nest more than 128 levels deep is a `manifest-nesting-too-deep` error. In the Rust API, the limits are `mesh_parser::MAX_NESTING_DEPTH` and `mesh_manifest::MAX_NESTING_DEPTH`, and at those limits the whole pipeline, from parsing to rendering, runs on a 2 MiB thread stack.

### Changed

- **Breaking, Rust API: `CompileResult` keeps its analysis, and is `#[non_exhaustive]`.** `compile_with` used to analyze a file against its template and keep only the diagnostics. Its `CompileResult` now also has `analysis: Option<mesh_analysis::Analysis>`: the resolutions, types and facts the analysis diagnostics came from. It's `None` from `compile` and for a file with a syntax error. Code that builds a `CompileResult` or destructures every field must change; code that reads its fields by name doesn't. Being `#[non_exhaustive]`, the next field won't break anyone.
- **The grammar is versioned with MESH.** `tree-sitter-mprx` moves from 0.1.0 to 0.3.0, as every crate does; it had stayed at 0.1.0 through v0.2.
- **The npm packages are renamed to the `@valance` scope:** `@valance/mesh-language`, `@valance/mesh-compiler` and `@valance/mesh-lsp`, with the Valance spelling. They are still unpublished placeholders. `@valance/mesh-lsp` no longer depends on a Node language server: it will only launch the Rust `mesh-lsp` binary.
- **Breaking, in principle: the nesting limits.** A file or manifest nested more than 128 levels deep, which v0.2 accepted if it was shallow enough not to crash, is now rejected. These are MESH's supported limits, resource limits of this implementation: neither MPRX nor manifest version 1 limits nesting, so the manifest's version doesn't change. No handwritten UI or manifest comes near the limit; to stay under it, flatten the file, or write deeply nested manifest types as named types.

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

[0.4.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.4.0
[0.3.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.3.0
[0.2.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.2.0
[0.1.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.1.0
