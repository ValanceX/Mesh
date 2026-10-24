# Changelog

All notable changes to MESH are recorded here. The project follows [Semantic Versioning](https://semver.org). Until 1.0, minor versions may include breaking changes.

## [Unreleased]

## [0.17.0] - 2026-10-24

Prepared, not yet published. Streaming: checking a source while it arrives (direction Bet 13, WP9). No behavior changes for a source that is checked whole: `check`, `compile` and `checkProgram` give what they gave in 0.16.0.

### Added

- **`openSession` in `@valancex/mesh-compiler`, and `mesh_compiler::stream` in Rust** (`repair`, `step`, `Session`): a stream session over a component model. `push` a chunk and get the diagnostics that are new (a mistake is reported once, at the first prefix that holds it), the diagnostics standing, what is open (`pending`), and, while no error stands, a **preview**: the part that has arrived, checked and compiled to a `template-v1`, to draw as a fresh render. `close` is exactly the batch check of the whole text, however it was cut (property-tested over 100,000 random prefixes and mutations in Rust and 3,000 through WebAssembly). The check is of the **repaired prefix**: the text cut back to its last complete token, with what is open closed, by the module and not by the host. Nothing is kept that was cut inside a start tag, an attribute string, an expression or a block header; a trailing run of text is kept and grows; an open `{#each}` is withheld until it closes; a start tag still being written is checked as far as it has got for what is final about it (a wrong prop is heard when its attribute is complete). A push has a budget of 50 ms of checker time (`budgetMs`): over it, the session ends with `check-budget-exceeded`. See `docs/manual/streaming.md`.
- **`mesh_stream`** in the WebAssembly module: one stateless export, the check of a repaired prefix.
- **`effect` and `title` on a command in the manifest** (`"read"`, `"write"` or `"destructive"`; a name for a person): annotations a host reads to ask before it runs a command a generated screen asked for. They change nothing in how a template is checked, compiled or rendered and are not part of a template's fingerprint. `mesh_manifest::{Effect, Command::effect, Command::title}`; the schema `manifest-v1` has both.
- **`generated/` in `@valancex/mesh-conformance`** (`examples/conformance/generated/`): six pages, eight planted mistakes and the digests of what a session reports at every prefix of each, recorded by the native runtime and reproduced through WebAssembly.

### Changed

- The manifest crate's `Command` has two new public fields (`effect`, `title`); it is `#[non_exhaustive]`, so a reader of it is unaffected.

## [0.16.0] - 2026-10-24

Published. Replay: the session log and the conformance logs (direction Bet 15, WP7). No runtime behavior changes: a program, a manifest and a snapshot render as they did in 0.15.0.

### Added

- **`replay/` in `@valancex/mesh-conformance`** (`examples/conformance/replay/`): the session-log format (`format` 1: line-JSON of `header`, `changes`, `event`, `tree` and `result` lines), seven recorded logs (a keyed list, nested composites, an `{#if}`/`{:else}`, rejected updates that leave the old render valid, number text, stale handlers, payloads and foreign handlers), and 1,000 random sessions of the keyed list with their digests. Every runtime must replay them exactly, by three routes: an `update_changes` chain, an `update` chain over whole snapshots, and a `render` of each snapshot. The native runtime (`crates/mesh-runtime/tests/replay.rs`) records and replays; the WebAssembly package (`packages/mesh-runtime/test/replay.test.mjs`) replays the same logs and gives the same digests for the random sessions, so the two runtimes agree on every session, not only on the routes within each.

## [0.15.0] - 2026-10-23

Prepared, not yet published. Units: the first state a component holds for itself (direction Bet 8, WP5). A manifest and a program that use none of it compile and render as in 0.14.0; one that has a `unit` is refused by an older MESH.

### Added

- **Units: state that belongs to one occurrence of a composite** (direction Bet 8, WP5). A component's manifest entry may have a `unit`: state fields with defaults, and commands. The state names join the component's scope and the commands its commands, so a template reads and invokes them like any others and checking needs nothing new. The runtime takes the state the host holds as an input beside the snapshot (`render`, `update`, `updateChanges`, and the Rust `render_units`, `update_with_units`, `update_changes_units`, `dispatch_from_units`); an occurrence with no entry has the defaults. A render lists the unit occurrences it met (`instances`), an intent for a unit's command names its occurrence (`instance`, and `controlled`), a `controllable` field takes the caller's prop when it is written, and `raise` resolves an event a unit announces to the handler its caller bound. A part of the tree that an update reuses is rebuilt when the state of a unit below it has changed. `render-v1` and the patches are unchanged. New codes: `manifest-unit-clash`, `runtime-unknown-instance`, `runtime-unknown-event`. A manifest with a `unit` is refused by an older MESH (`manifest-unknown-property`); one without is read as before. The WebAssembly module's exports changed (internal).

### Changed

- **The release publishes `@valancex/mesh-conformance`.** The conformance vectors package (the renderers' test dependency; PORT depends on `^0.14.0`) was never in the release workflow, so 0.14.0 was published without it. `.github/scripts/publish.mjs` now lists it as the ninth package and the workflow packs it. Its first version is published by hand (a package that does not exist cannot have a trusted publisher): `cd packages/mesh-conformance && npm publish --access public`, at the version of the release.
- **The runtime's WebAssembly is optimized after linking** with binaryen's `wasm-opt -O2` with inlining off (`@valancex/mesh-runtime`'s `build:wasm`): 817,965 bytes become 700,870, gzipped 248,984 become 234,818, and brotli, which is what a site ships, 181,837 become 176,389 (-3.0 percent). The flags are chosen by brotli: `-Oz` (the compiler's) makes this module's brotli 1.4 percent larger. The runtime's tests, the conformance vectors and the replay hashes are unchanged.

## [0.14.0] - 2026-10-18

Prepared, not yet published. The first new types and the first new template construct since 0.10: enum types, presence tests and defaults (`template-v2` for the two expressions), snippets, and the limits and fixes of direction work package 0. In the free window: each break names its replacement in a diagnostic. A program, a manifest and a template that use none of the new constructs compile and render as they did in 0.13.0, with the same fingerprint, tree and keys (tested against the published 0.13.0).

### Changed

- **The compiler's WebAssembly is optimized after linking** with binaryen's `wasm-opt -Oz` with inlining off (`@valancex/mesh-compiler`'s `build:wasm`; `binaryen` 132.0.0 is a development dependency, and `MESH_NO_WASM_OPT=1` skips it): 989,551 bytes become 735,048, and the gzipped 320,778 become 301,420 (the size budget stays 330,000). Every test passes on the optimized module, and the replay hashes are unchanged. With inlining on, the file is smaller still and its gzip is larger, which is why it is off.

### Added

- **`runtime-nesting-too-deep`.** A render nests at most 512 levels (an element, a composite's expansion, a fragment, a slot or a repeated item is one each). About 680 nested composites used to crash the WebAssembly runtime with `MeshInternalError`; now it is a diagnostic and nothing is rendered. Not host-configurable.
- **`input-too-large`, `too-many-elements`, `too-many-attributes`.** A source over 64 KB, more than 5,000 elements, or more than 64 attributes (event bindings count) on one element is one error at the first place past the limit, before anything else is checked. **Sources over 64 KB that used to check now fail**; split them into components.

### Fixed

- **A compiled `template-v1` is read at the documented depth.** `serde_json`'s own limit of 128 JSON levels refused a template at about 42 nested elements (an element is three levels), though 128 are documented. The reader now bounds the depth itself at 1,024 JSON levels and refuses deeper with `assembly-malformed-template`.
- **The checker is linear in siblings and attributes.** Every position lookup scanned its line, so a source on one line (generated or minified UI) cost the square of its size: 8,000 siblings took 4.9 s and 10,000 attributes 15 s in WebAssembly; they now take about 0.2 s. `SourceMap` answers by search.
- Conformance READMEs no longer say MPRX cannot express conditionals or repeats.

### WP2 step 1: enums, presence, defaults

The language's first new types (direction Bet 4). A compatible addition: a manifest and a program that use none of it keep their meaning, their fingerprints and their render trees byte for byte. An older MESH reading a manifest that uses `enum` reports `manifest-unknown-kind`.

### Added

- **`enum` types.** `{ "kind": "enum", "values": ["info", "warning", "danger"] }`: a string that is one of 1 to 256 distinct values. An enum fits `string`; an enum fits another that has all its values; the common type of two enums is the union, and of an enum and `string` is `string`. `string` does not fit an enum, except as a **string literal** at an attribute value, an operand of `==`/`!=`, a `?:` branch, an array element or the right of `??`. New codes `manifest-enum-too-large`, `manifest-duplicate-variant`, `unknown-variant` (with a did-you-mean help line).
- **`has(x)` and `x ?? y`.** `has(x)` is `true` unless `x` is absent; `x ?? y` is `x` when present, else `y` (lazy, between `?:` and `||`). `has` is not reserved. New warnings `redundant-presence-test` and `redundant-default`. The expression `has` and the binary operator `coalesce` are **`template-v2`** (see Changed). 
- **`default` on props.** A prop may declare `"default": <value>`; a composite occurrence that leaves the prop out binds the default instead of absence (primitives ignore it). Binding soundness (rule 4b) is checked with the prop's own type. The model fingerprint includes a default, and is unchanged for a prop without one.
- The runtime checks an enum where host values enter (snapshot fields, event payloads, values read through `any`, props, command arguments) with the existing `runtime-value-mismatch`, `runtime-prop-mismatch` and `runtime-argument-mismatch`. `render-v1` and the patch format are unchanged.
- Conformance vectors `examples/conformance/enums/`, with their own manifest.

### Added (snippets, Bet 5)

- **Snippets.** `{#snippet name(params)} … {/snippet}` declares a piece of template with parameters, and `{@render name(args)}` renders one, wherever an element may be. A snippet declared as a direct child of a component's occurrence is passed as the prop of that name; content between the tags of a component that declares a `children` snippet prop is passed as `children`; any other is a local name for the children around it, so a snippet can render itself. A snippet reads the scope of the template that declares it. Slots (`mesh-slot`, `mesh-fill`) keep working.
- **The `snippet` type kind**, `{ "kind": "snippet", "params": [...] }` (`snippet<A, B>`): a composite's prop or scope name only (`manifest-snippet-misplaced`), parameters compared contravariantly with exact arity, never an `any`. `children` is reserved: a `snippet` with no parameters (`manifest-children-not-snippet`). The fingerprint hashes a snippet as its ordered parameters.
- **Codes:** `unknown-snippet`, `snippet-arity-mismatch`, `duplicate-snippet`, `children-conflict`, `manifest-snippet-misplaced`, `manifest-children-not-snippet`, `assembly-snippet-to-primitive`, `assembly-snippet-root-scope` and `runtime-snippet-depth`.
- **Identity:** a render site adds the step `0x08`; a subtree that renders a snippet is never kept on its own memo (as with a slot), and the vectors in `examples/conformance/snippets` fail without that rule. Snippets render at most 64 levels deep. `render-v1` and the patch format are unchanged; a template that has a snippet is `template-v2`.
- **A forwarder (U13).** A template may be only `{@render children()}`: the document is read as `<mesh-fragment>{@render …}</mesh-fragment>`, so a composite can forward what its caller wrote without a node of its own. The grammar's `source_file` is an element or a render tag; the output is `template-v2`; a program's root cannot be one (`assembly-root-fragment`, whose message now says so). Vectors: `examples/conformance/forwarder`.
- **The language server** hovers a snippet's name, and completes snippet names after `{@render `, a snippet's parameters and an `{#each}` item where a value starts, and the members of a closed set inside the quotes of a prop's value or beside `==` and `!=` (the new candidate kinds `Snippet`, `Parameter` and `EnumValue`).
- **Go to definition** from a snippet's name to its declaration, and **signature help** in a render's arguments (`textDocument/signatureHelp`, triggers `(` and `,`).
- **Optional snippets (U19).** A render of a snippet whose type is optional is `possibly-absent-snippet` unless it is in the first branch of an `{#if}` that has `has(name)` (`&&` allowed); the completion, hover and signature help of an optional snippet say so. A MESH from before 0.14 reads such a manifest as `manifest-unknown-kind` (tested against 0.13.0).
- **Passing a local snippet along** as a prop value is specified and has vectors (`examples/conformance/passing`).
- **Not yet:** contract inference for snippet props (declare them in the manifest), and signature help for commands.

### Changed

- **`template-v2`.** The stability policy lets a `-v1` format gain only optional properties, so the expression `has` and the operator `coalesce` are a new version, not an addition to `template-v1`. `template-v2` is a superset (`schemas/template-v2.schema.json`); the compiler writes it **only** for a template that uses `has` or `??`, and every other template is written as version 1, byte for byte as before. The runtime and `mesh-template` read both; a version 1 document that holds a version 2 construct is `assembly-malformed-template`; a runtime from before this reads only version 1 and refuses version 2 with `assembly-unsupported-format-version`. `schemas/template-v1.schema.json` no longer has `has` or `coalesce` (they were never released).
- `possibly-absent-access` no longer says MPRX has no presence test.

## [0.13.0] - 2026-10-17

A change to how MPRX reads text: a space written between two interpolations is content. Nothing else in the language, the compiler, the runtime or any document format changes. See the [v0.13 release notes](./docs/releases/v0.13.md).

### Changed

- **A space between two interpolations is content.** `<text>{first} {last}</text>` rendered `AdaLovelace`, because the space is a whitespace-only text child and those are formatting (§3). A whitespace-only text child with no line break, between two interpolations, is now kept, so it renders `Ada Lovelace`. A line break stays formatting (`{a}` and `{b}` on separate lines read `ab`), and so does whitespace next to an element, a block, a slot or an edge; `{" "}` still writes a space anywhere. **A change in rendered text**, so a minor version; a program with no such space renders exactly as before, with the same keys and handler identifiers. See the [design note](./docs/superpowers/specs/2026-10-16-mprx-whitespace-between-interpolations.md).

## [0.12.0] - 2026-10-13

The editor formats and indents MPRX. Nothing in the language, the compiler, the runtime or any document format changes. See the [v0.12 release notes](./docs/releases/v0.12.md).

### Added

- **Formatting and indentation.** `mesh-lsp` answers `textDocument/formatting`, `rangeFormatting` and `onTypeFormatting` (after a newline), from the new `mesh-format` crate: indentation by the nesting of elements and blocks, one attribute per line in a tag written over several lines, nothing but whitespace between tokens changed (the formatter checks its own result and refuses a source that does not parse). The VS Code language configuration gains indentation and Enter rules, and the extension turns on `editor.autoIndent` and `editor.formatOnType` for MPRX.

### Changed

- All crates and packages move to 0.12.0 together.

## [0.11.0] - 2026-10-13

Control flow becomes syntax. Conditional and repeated structure are written as blocks, `{#if}` and `{#each}`, and the reserved elements they replaced (`mesh-if`, `mesh-each`, `mesh-switch`, `mesh-case`, `mesh-default`) are errors. **Breaking before 1.0**: a project that uses those elements must rewrite them as blocks, and moves its runtime and compiler packages together. The runtime, `template-v1`, `render-v1`, node identities and handler identifiers are unchanged. See the [release notes](./docs/releases/v0.11.md).

### Added

- **Blocks.** `{#if condition}`, any number of `{:else if condition}`, an optional `{:else}`, and `{/if}`; and `{#each items as item (key)}` … `{/each}`, whose key is mandatory. A branch or body that is exactly one element is that element; anything else is wrapped in a `mesh-fragment` automatically. A block cannot be the document, text cannot contain `{#`, `{:` or `{/`, and a block counts as three levels toward the nesting limit. A chain of `{:else if}` branches replaces `mesh-switch`.
- **Built-in constructs.** `mesh-if`, `mesh-each` and `mesh-fragment` need no declaration in a manifest (`mesh_manifest::Manifest::component`, `BUILT_IN_COMPONENTS`); one that declares them is used as declared, and the model fingerprint is unaffected. `compileProgram` no longer declares them for a program that uses blocks; it still declares `mesh-slot` and `mesh-fill`.
- **Diagnostics.** `retired-element` (a reserved element written as an element; the message names the block to write) and `missing-repeat-key`. Syntax errors in blocks keep `syntax-error` with specific messages: a block never closed, a closer or `{:else}` with no open block, a malformed opener, a block as the document.
- **Binding.** The name `as` binds is a binding: analysis resolves it for typing and scoping (`Target::Binding`), hover shows its type, and go-to-definition goes to where it is bound.
- **Tooling.** The Tree-sitter grammar, its highlight queries and the TextMate grammar recognize blocks; completion offers values after `{#if`, `{:else if` and `{#each`.

### Changed

- **Breaking: the reserved elements are errors.** `<mesh-if>`, `<mesh-each>`, `<mesh-switch>`, `<mesh-case>` and `<mesh-default>` are `retired-element`. Diagnostics about a block name the block ("the condition of `{#if}`", "the key of `{#each}`").
- **Component listing offers nothing for a block.** Tag completion no longer lists a model's `mesh-if`, `mesh-each`, `mesh-switch`, `mesh-case` or `mesh-default`; they resolve no component.
- All crates and packages move to 0.11.0 together.

### Removed

- **`invalid-switch` is no longer reported.** A chain of branches cannot be malformed as a `mesh-switch` could. The code stays in the catalogue, because codes are never reused.

## [0.10.0] - 2026-10-10

Fine-grained updates and composition: the runtime updates a render from a new snapshot or from changes and returns patches; composites have children, named slots and events; a program's composites need no declared contract; and `mesh-switch` chooses among alternatives. The public APIs were reviewed against the API design principles first: see [the review baseline](./docs/superpowers/specs/2026-10-09-api-review-baseline.md). See the [v0.10 release notes](./docs/releases/v0.10.md).

### Added

- **Fixed: text on either side of a conditional or a repeat that chose nothing is one run.** A render tree's text runs are maximal (`docs/manual/runtime.md`), but `a<mesh-if>…</mesh-if>b` with the conditional off gave two runs side by side, which a server renders as one text node and a client could not match. They are now merged, whatever made them adjacent, named by the first.
- **`mesh-fragment`**, content without a node: a composite's template, a `mesh-if` alternative, a `mesh-case` body or a `mesh-each` item can be text or several nodes by writing a fragment. MESH wraps nothing on the author's behalf: a render node exists only where a template wrote an element. A fragment is declared in the model like `mesh-slot` (no props, no events; `compileProgram` declares it when used), adds an identity step (`0x07`), and cannot start the program's root (`assembly-root-fragment`). Provisional. See [Fragments](./docs/manual/templates.md#fragments).
- **Named slots.** `<mesh-slot name="header" />` in a composite's template, and `<mesh-fill slot="header">…</mesh-fill>` as a direct child of an occurrence, place different content in different places; children outside a fill go to the default slot, as before. `mesh-fill` is declared in the model like `mesh-slot` (a required string prop `slot`), and `mesh-slot` gains an optional `name`. A named slot's identity step is the default slot's with the name as its component, so no key in a program without named slots changes. `assembly-composite-children` now covers a fill for a slot that isn't there, two fills for one slot, a fill outside a composite occurrence, and loose children without a default slot. See [Named slots](./docs/manual/templates.md#named-slots).
- **`mesh-switch`**, for choosing among several alternatives: `mesh-case` elements with a boolean `when`, and optionally a last `mesh-default`. The first case that holds is rendered. The compiler writes a switch as the conditionals it stands for (one `mesh-if` per case, true only when no earlier case is), so `template-v1`, the runtime, identity and updates are unchanged: each case is a conditional site of its own. Provisional, like `mesh-if`. New diagnostic: `invalid-switch`. `compileProgram` declares `mesh-switch`, `mesh-case`, `mesh-default` and the other reserved tags (`mesh-slot`, `mesh-if`, `mesh-each`) that a program uses and the manifest does not. See [Switch](./docs/manual/templates.md#switch).
- **Inferred composite contracts.** `compileProgram` writes the manifest entry of every listed component the manifest doesn't declare: its props from the arguments its occurrences pass (with their types; a prop some occurrences leave out is optional), its events from the handlers it forwards (with the forwarded argument's type, so `$event` is typed through every level), and `mesh-slot` if a template uses it. Nobody writes composites into the manifest any more; a declared component is used as written, so a custom contract is still a declaration. `program.model` is the manifest with the entries added. Rust: `mesh_compiler::infer::infer_components`; the module's new `mesh_infer` export is internal. No new diagnostic: the compile that follows reports problems as for a hand-written manifest. See [Inferred contracts](./docs/manual/templates.md#inferred-contracts).
- **A `hint` on runtime diagnostics** for the codes where one action reliably applies (`runtime-missing-value`, `-value-mismatch`, `-unknown-field`, `-handler-other-program`, `-unknown-handler`, `-invalid-key`, `-duplicate-key` and the three changes codes): an optional `hint` string beside `message` in the runtime diagnostics document (the schema allows it; `mesh_runtime::diagnostic::hint(code)` in Rust, `RuntimeDiagnostic.hint?` in TypeScript). Additive, under the rule now stated in [API stability](./docs/manual/stability.md): a `-v1` document may gain optional properties, and a consumer ignores properties it doesn't know.
- **`RenderVersion`**, the branded type of `render.version` and `Changes.base`, so a version can't be confused with another number. `updateChanges` works on any render, including one `render()` made: the module derives it again under its version, so no option is needed to make a render a base for changes. `render()` keeps nothing in the module, as in 0.9.0.
- **Stable codes on the JavaScript packages' errors, and `using` for renders.** `MeshInternalError` (`internal-error`), `MeshVersionError` (`version-mismatch`) and a new `MeshUsageError` (`invalid-argument`, `not-a-render`, `render-gone`) carry a `code` to match on, in both `@valancex/mesh-runtime` and `@valancex/mesh-compiler`. `MeshUsageError` extends `TypeError` and replaces the bare `TypeError`s the packages threw, so existing `catch` and `instanceof TypeError` keep working. A `Render` has `[Symbol.dispose]()`, the same as `release()`, where the platform has it. The package READMEs now say that calls run one at a time in the order made and can't be cancelled. `init` rejects with a coded error too: `MeshUsageError` (`invalid-argument` for a source of the wrong type, `module-unavailable` when the module can't be read or fetched) or `MeshVersionError` (bytes that aren't a WebAssembly module); an operation that is given a `Render` this package didn't make names itself in the message. `RuntimeDiagnostic.hint` is in the compiler package's types too. See [API stability](./docs/manual/stability.md) for what each surface promises.
- **Update from changes.** `mesh_runtime::update_changes(&previous, &changes, verify)` and `updateChanges(render, { base, changes }, { verify })` take the edits to a render's snapshot (`set`, `insert` and `remove` at paths) instead of a whole snapshot, so an update costs the edits and what depends on them: the snapshot is neither encoded nor validated again, only the values the edits give are validated, and what they don't touch is shared with the previous render. The edits define the new snapshot, are applied all or none, and name the render they are for by its **version** (`render.version`, `Render::version()`; new for each render): changes for any other render are refused. With `verify` (the whole snapshot the host believes it has) a difference is refused too; it is for tests and development. `diff(previous, next)` in `@valancex/mesh-runtime` computes the edits between two snapshots, for a host that doesn't know what it changed. New diagnostics: `runtime-changes-base-mismatch`, `runtime-invalid-change`, `runtime-changes-disagree`. On a keyed list of 10,000 items with one changed: 13 ms in Rust (21 to 42 for `update`), and about 22 ms from JavaScript (about 60 ms for `update`, 460 for `render`). In JavaScript a render made from changes keeps its snapshot only in the module, so after `release()` it can't be updated or dispatched with; `updateChanges` accepts any render; the module derives one it doesn't hold. `src/changes.ts` reads host values (to compare them), a second named exception in the package's review test, which checks nothing the runtime does depends on it. See [Changes](./docs/manual/runtime.md#changes). The module's new `mesh_update_changes` and `mesh_dispatch_kept` exports are internal like `mesh_update`.
- **Composite events.** A composite may declare events, and its template forwards one by naming it as a handler (`on.tap={select(count)}`, where `select` is an event it declares, not a command): the occurrence binds it as any component's event (`<card on.select={pick($event)} />`), in the scope of the template the occurrence is in, and the event goes out through every composite on the way until a command handles it. No syntax is added (a handler's name is looked up among the commands, then the component's events; the compiler checks the payload argument). A forward no occurrence binds isn't in the render tree, so event resolution goes on past it. The runtime's declared events list only the bindings of commands. **Changed:** `assembly-composite-event` (rule 6) is now about a composite that declares an event *and* a command of one name; declaring an event no longer breaks it. See [Composite events](./docs/manual/templates.md#composite-events). 
- **Composite children and the slot.** A composite's template can place the children of an occurrence with the reserved element `mesh-slot` (declared in the model like `mesh-if`, with no props). The children are the **caller's**: evaluated in the scope they were written in, calling the caller's commands, and named by paths of their own (a new step kind, `0x06`, for the slot), so the same content in two occurrences never shares a key. A composite takes children exactly when its template has a slot (`assembly-composite-children` otherwise; its message changed); a slot may forward the content to another composite; text beside a slot is one run; and a malformed slot (two in a template, one with children, as a root, or as the direct child of a `mesh-if` or `mesh-each`) is a malformed template. `update` keeps working: a node that places a slot is rebuilt, never kept, and the content is kept or rebuilt by its own memos. No manifest or `template-v1` change. See [Children and the slot](./docs/manual/templates.md#children-and-the-slot).
- **`update`**, a third runtime operation: `mesh_runtime::update(&previous, &snapshot)` and `update(render, snapshot)` in `@valancex/mesh-runtime` render a new snapshot of the same program, reuse every node's props and text whose inputs are unchanged, and return the new render with the patches that turn the previous tree into its tree. See the [runtime manual](./docs/manual/runtime.md#update).
- **`render-patch-v1`** ([`schemas/render-patch-v1.schema.json`](./schemas/render-patch-v1.schema.json)): `setProp`, `removeProp` and `setText` for a kept part; `insert`, `remove` and `move` for parts among their siblings, matched by key (spec §9.10), so a conditional switching, a list gaining, losing or reordering items is a few operations and not a rebuild; and `replace`, the only operation of its list, for a tree nothing else can turn into the next. Applying a list to the previous tree gives exactly a full render's tree; the runtime's tests check this over random snapshots (inserts, deletes, reorders and optional props among them) and validate every list against the schema.
- **An update that reuses what hasn't changed.** A render's tree is now shared (`Rc`) and each node remembers what it was computed from, so `update` keeps a node whose subtree's inputs are identical as the very same node without looking inside it; a repeat remembers its items, and reuses an identical item without evaluating its key or hashing its path; and the diff skips shared nodes. On a keyed list of 10,000 items with one changed: 7 expressions evaluated, not 60,001, and 21 ms, not 91 ms, in Rust; 77 ms, not 481 ms, from JavaScript. What remains is the size of the snapshot (it is encoded, decoded and validated each time). **Breaking, in Rust:** `Tree::root` is an `Rc<Node>` and `TreeChild::Node` holds an `Rc<Node>`; code that reads a tree is unchanged by auto-deref, code that builds one (a render does, a host doesn't) is not. `update_with` takes the snapshot instead of copying it.
- **The module returns an update's patches, not its tree.** The JavaScript package applies them to the previous tree (`src/patches.ts`), so an update costs what changed and not the size of the tree; the new tree is frozen and shares every unreached part with the previous one. This is the one place the package reads a tree, an exception to its "encodes, and never judges" rule that its review test (`test/no-evaluation.test.mjs`) now names, and checks reads no value. `scripts/bench-update.mjs` measures it.
- **Renders kept in the module.** The WebAssembly module keeps the render `update` or `updateChanges` returns, under a handle, so the next `update` from it doesn't derive it again, and `render.release()` ends the copy: **the module is no longer stateless**, which it was until now. `render()` and `dispatch()` still keep nothing, a render that was released or came from a module since replaced updates by deriving itself again (except a render `updateChanges` made, which has no snapshot outside the module), and a render that is never released is released when it is garbage collected. The package's memory test releases every render in a chain of ten thousand updates and checks that memory doesn't grow. The `mesh_update` export takes the handle and `mesh_release` ends it; both are internal to the package, which checks for them: a `@valancex/mesh-runtime` is built with its own module.
- `mesh_runtime::evaluations()` (hidden), a count of evaluated expressions, for the update benchmark and tests.

### Changed

- **The runtime's `init()` is optional in a browser.** Without it, the first call loads the packaged `mesh-runtime.wasm` from next to the runtime's code (`new URL("./mesh-runtime.wasm", import.meta.url)`), as Node always did. `init(source)` is unchanged and still decides which module is used. If the automatic load fails, the error says so, keeps the original failure as its `cause`, and names `init()` and, for a Vite 5 to 7 development server, excluding `@valancex/mesh-runtime` from dependency optimization.

## [0.9.0] - 2026-10-04

Build-time program assembly: the compiler package compiles a program's components and returns the program's parts. See the [v0.9 release notes](./docs/releases/v0.9.md).

### Added

- **`compileProgram`** in `@valancex/mesh-compiler`: compiles a program's components against one manifest, checks the program they make with `checkProgram`, and returns the program's parts (`{ model, root, templates }`, the templates as text) for the runtime. It composes `compile` and `checkProgram` and adds no rule of its own; a failure is the existing diagnostics documents.

## [0.8.0] - 2026-10-04

Declared events: the runtime says which events a program declares, so a host can compare them with what it handles. See the [v0.8 release notes](./docs/releases/v0.8.md).

### Added

- **Declared events.** The runtime says which events a program declares: `mesh_runtime::declared_events(&program, model)` in Rust, and `declaredEvents({ program, model })` in `@valancex/mesh-runtime`. Each declared event is one event binding in one of the program's templates, with its declaring `component`, `event`, `command` and source `span` (`mesh_runtime::DeclaredEvent`). They are the validated program's own, not a render's: those in composite templates, in inactive `mesh-if` alternatives and in `mesh-each` bodies are included, and nothing is deduplicated. The program is validated as `check_program`, render and dispatch validate it, and an invalid program gives the same diagnostics. Documented in the runtime manual ("The declared events"). The runtime WebAssembly module has a new export, `mesh_declared_events`, which the package now requires. Additive: `check_program`, render, dispatch, the compiler and `template-v1` are unchanged.

## [0.7.0] - 2026-10-02

Node identity for programs whose structure varies: the locked contract (spec §9.10), and the provisional conditional and repeated structure that implements it. See the [v0.7 release notes](./docs/releases/v0.7.md) for an overview.

### Added (provisional)

- **Conditional structure, `mesh-if`.** The runtime gives one reserved component, `mesh-if` (declared in the model with a boolean `when`), conditional meaning: one or two element children are its alternatives, and it is never a render node. Each alternative is a distinct site, and a conditional is one slot among its siblings, so a node's key no longer depends on where it lands among the rendered children. Alternatives add a `0x04` step to a key's path. Dispatch accepts a handler only if its node is in the render.
- **Repeated structure, `mesh-each`.** The runtime gives a second reserved component, `mesh-each` (declared in the model with `items`, `as` and `key`), repetition meaning: its one element child is rendered once per item, each node under a `0x05` step holding the author-declared key (a string or a finite number; `"1"` is not `1`; `0` is `-0`), never the item's index. Keys are evaluated at render, so they are a function of the program and the snapshot; a missing, invalid or duplicate key fails closed with the new codes `runtime-invalid-key` and `runtime-duplicate-key`. The analysis binds `as` to the item's type for `key` and the child, and requires the key's type to be a string, a number or `any` (`type-mismatch`: "a repeat's key must be a string or a number"). Dispatch, for a program with a repeat, records each handler's scope during a render of the stored inputs and evaluates there, so it finds the item by identity, not position.
- **Identity conformance vectors** (`examples/conformance/identity/`, C1–C10, and `identity/repeat/`, C11–C22): hand-written render-v1 before/after trees with each node's identity and the expected kept, created, removed and moved sets, handler expectations, and the pairing that matching by position would make. They need no MPRX and no compiler. The repeat vectors are also checked against the runtime.

Both are **provisional**: the spelling is not the language's decision, no MPRX syntax changed, and the key encoding is an implementation detail. A program that uses neither has exactly the keys and handler identifiers it had in 0.6.0. render-v1 and template-v1 are unchanged.

### Changed

- **A manifest that declares a component named `mesh-if` or `mesh-each` gives it these meanings.** No earlier manifest is known to; one that does would now be refused or render differently.
- **Rust API:** `mesh_analysis::Expectation` has a new variant, `RepeatKey`, and `mesh_runtime::RuntimeCode` has `INVALID_KEY` and `DUPLICATE_KEY` (in `RuntimeCode::ALL`). Code that matches `Expectation` exhaustively must handle the new variant.
- **Diagnostics reference:** `runtime-invalid-key` and `runtime-duplicate-key` are documented in `docs/manual/diagnostics.md`.

### Documented

- **Node identity contract (spec §9.10).** An identity is a sequence of (site, instance) steps, a site being a template place and an instance a key the application declares at a repeated site, so keys are never rendered positions or values MESH infers; equal identity is the same node, whatever order its siblings come in; identities are unique and a duplicate or invalid key fails closed; a key's type is checked in the template where types can say it, and every value at render; an absent node's occurrence ends and a returning one is new, identity being a name and not a handle to a realization; a handler's identity is its node's identity and its event, and dispatch derives a repeated item's scope from the program and the snapshot; and identity stays within one program, so update and draw keep their meaning.
- The runtime manual, the rendering guide and the spec no longer say a program's structure never changes: that is true of a static program, and a program that uses `mesh-if` or `mesh-each` varies.

## [0.6.0] - 2026-09-27

The semantic questions PORT's first renderer raised, settled. See the [v0.6 release notes](./docs/releases/v0.6.md) for an overview.

### Added

- **`propText` in render trees.** A node now carries, as `propText`, the MESH text (spec §9.7.7) of each prop whose value is a number, a boolean or `null`: `{ "value": 42, "busy": false, "note": null }` comes with `{ "value": "42", "busy": "false", "note": "null" }`. The runtime makes it, with the same rule as text runs (§9.7.7.1 for numbers), so a renderer that puts such a prop in a slot that holds only text never converts a value itself. A string prop has no entry (its text is its value), a list or record prop has none (it has no text), and an absent prop has neither a value nor a text, unlike a `null` one. The member is present only when some prop has an entry. `render-v1` keeps `version: 1`: its schema lets a later MESH add properties, and renderers must ignore properties they don't know. The schema now lists `propText`; every v0.5 tree is still valid against it.
- **Event resolution** (spec §9.9). One interaction reaches at most one binding: from the innermost interacted node towards the root, the first node whose primitive has the interaction's applicable event, and which binds it, receives the interaction, and no ancestor after it. This is MESH's rule, not a target's event propagation, and MPRX has no syntax to change it. A primitive's event contract (what each of its events means) is MESH-level and the same on every target; a PORT maps its target's interactions onto it, and resolution is given the resulting applicable event per primitive. v0.6 adds no primitive catalog, interaction vocabulary or manifest change for it. Rust API: `mesh_runtime::resolve(&tree, &Interaction)`, the reference implementation, returns `Ok(Some(Resolved))` (the receiving node's key, the event and the handler identifier), `Ok(None)` when nothing qualifies, or `Err(ResolveError::UnknownTarget)` for a target key not in the tree. It evaluates nothing and needs no target. `Interaction`, `Resolved` and `ResolveError` are new; the last two are `#[non_exhaustive]`.
- **Conformance vectors,** `examples/conformance/`: real programs, their committed trees, and named cases for values and `propText` (`values/`, including one pinning absent, `null` and a string as three distinct node shapes) and for event resolution (`events/`), in JSON and MPRX only, so a renderer in any language can test itself against them without MESH's code. They're checked natively and through `@valancex/mesh-runtime`, and native and WebAssembly give byte-identical results for them, and for every finite number of the normative table as a prop's `propText`.
- **`@valancex/mesh-runtime`: `RenderNode.propText`,** optional, in the exported types.

### Changed

- **The spec.** §9.7.7 is now the text of a value, for props as well as content; §9.7.8 extends "no text" to list and record props; §9.8.2 separates a value's text from the value; §9.8.7, new, says how a renderer may realize a value: natively, in a slot of its kind that holds it exactly, or as its MESH text in a slot that holds only text, and nothing else (no platform coercion, host formatting or JSON); §9.9, new, is event resolution.
- **Breaking, Rust API: `mesh_runtime::Node` has a new field, `prop_text`, and is now `#[non_exhaustive]`,** so code outside the crate can no longer build one with a struct literal or destructure it exhaustively. Reading its fields by name is unaffected, and the next field won't break anyone.
- **Renderers must resolve events** (§9.9). A renderer that lets one interaction reach more than one binding, as a DOM renderer relying on bubbling does, no longer conforms. No existing MESH program changes meaning: none in v0.5's examples, fixtures or test programs nests bindings.

### Fixed

- **The documents' contradictions about text,** found while settling PORT's questions: the rendering guide said numbers arrive as text, which holds only for text runs; the v0.5 outline's I11 both forbade converting a value to text outside the runtime and called setting a DOM attribute from any value "drawing" (ARCHITECTURE rule 9 now says the runtime makes every text and renderers consume it); the reference renderers' JSON printouts are now labelled as inspection output, not realization; and the runtime manual's example tree held values its components' manifest doesn't allow.

## [0.5.0] - 2026-09-26

MESH renders: templates, and the MESH runtime, natively and as `@valancex/mesh-runtime`. See the [v0.5 release notes](./docs/releases/v0.5.md) for an overview.

### Added

- **Two guides,** [Integrating MESH with NEXUS](./docs/guides/integrating-mesh-with-nexus.md), for a host, and [Rendering MESH output](./docs/guides/rendering-mesh-output.md), for a renderer, each with an example a test runs. The release workflow now publishes `@valancex/mesh-runtime` too, eight packages in all.

- **The slice,** `examples/slice/`: a page that uses the composite `user-card` twice, with its committed tree, rendering, intent and changes, checked natively and in Node.
- **`@valancex/mesh-runtime`: the MESH runtime in JavaScript,** as a new package with no dependencies. `render({ program, model, snapshot })` returns `{ render }`, whose frozen `tree` a renderer draws, or `{ diagnostics }`; `dispatch(render, handler, payload?)` returns `{ intent }` or `{ diagnostics }`, against the snapshot the render was made from. It runs the Rust runtime in WebAssembly (`mesh-runtime.wasm`, 184,751 bytes gzipped), and only encodes and transports values: every JavaScript value, a `Map`, a hole, NaN or a cycle included, reaches the runtime, which judges it. It works in Node 22 and later and in browsers (after `init()`), and exports the render tree, intent and runtime diagnostics types.
- **`mesh check-program`:** checks a program of compiled templates (`--model`, `--root`, then the templates in order): the manifest, each template, their fingerprints and the assembly rules. It writes nothing. Its diagnostics, codes and locations are exactly what the runtime's `render` reports for the same program, because it runs the runtime's own program validation. `--format json` prints the runtime diagnostics document.
- **`@valancex/mesh-compiler`: `checkProgram({ model, root, templates })`,** which returns the document `mesh check-program --format json` prints, and exports the `RuntimeDiagnosticsDocument` type and related types for it. The compiler's module grows from 185,372 to 246,714 bytes gzipped, within its 300,000-byte budget.
- **Rust API: `mesh_compiler::check::program(model, root, templates)`,** the program check, and `mesh_runtime::encoding`, the byte encoding JavaScript hosts' values and template lists cross into WebAssembly in, with its decoder. `mesh_runtime::dispatch_from` dispatches from a render's program, model and snapshot, for a host that keeps those instead of the `Render`. An unsupported value's diagnostic now names its kind in words (a `Map` object, a value that contains itself).
- **Rust API: the `mesh-runtime` crate,** MPRX's one evaluator, natively (spec §9.7, §9.8; `docs/manual/runtime.md`). `render(program, model, snapshot)` validates the program (the model, each template, fingerprints, the assembly rules), then the snapshot (every mismatch, in path order), then evaluates the root's template, expanding composites, to a `render-v1` tree with a key for every node and text run and a handler identifier for every event binding. The result, `Render`, keeps its own copies of the program, model and snapshot; `dispatch(&render, handler, payload)` validates them again with the handler identifier and payload, and returns the command intent, against the render's snapshot, never a newer one. Errors are `RuntimeDiagnostic`s, rendered by `to_json` as `runtime-diagnostics-v1`; `check_program` runs program validation alone. `HostValue` and `HostRecord` hold what a host supplies, including what the boundary refuses, and `HostRecord::from_json` reads a snapshot. `number_to_text`, MPRX's text for a number (§9.7.7.1), is MESH's own exact digit generator: the shortest digits that read back, the nearest of those, the even one on a tie. It reproduces every row of the normative table, and uses no float formatter.
- **Rust API: `SourceMap` moved to `mesh-syntax`,** so the runtime needn't depend on the parser. `mesh_compiler::SourceMap` re-exports it, so its path still works.
- **`@valancex/mesh-compiler`: `compile()`.** It takes `check()`'s input, with the model required, and returns `{ diagnostics, template? }`: the document `check()` returns and, only when it has no error, the template, exactly as `mesh compile` reports and writes them. The package exports `Template` and related types for it.
- **`mesh compile`:** checks a file as `mesh check --model` does, with the same diagnostics and exit status, and, only when it finds no error, writes the component's template (`template-v1`) to `--output` or stdout. With an error it writes nothing. `--format json` prints the diagnostics document on stdout, and needs `--output`.
- **Rust API: compiling,** `mesh_compiler::check::template(source, &model)`: the check `check::source` does, and, only when it finds no error, the source's template (`template-v1`). Warnings don't stop it. `Model::fingerprint` gives the fingerprint templates compiled against a model carry.
- **Two new check errors** (spec §9.7): `content-not-text`, for an interpolation in content whose type is a list or a record (or an optional of one), which has no text form; and `number-literal-out-of-range`, for a number literal too large to be a finite binary64 value (309 digits or more), reported with or without a component model. `mesh-lsp` reports both too.
- **Rust API: the `mesh-template` crate,** the template format in Rust: `from_json` reads a `template-v1` document, refusing one that is malformed (with every problem, at a JSON Pointer) or of an unsupported version; `to_json` writes one, compactly and deterministically; and `fingerprint` computes a manifest's model fingerprint, which depends on the model's meaning only. It has no parser, so the runtime can depend on it. `serde_json` is now built with `float_roundtrip` across the workspace, so every JSON number reads as its exact nearest binary64 value.
- **MPRX's evaluation is specified** (`docs/MPRX-SPEC.md` §9.7): values and absence, the one runtime type relation (*fits*), IEEE 754 numbers with a truncated `%`, strict equality, evaluation order, the runtime checks, and text. Number-to-text conversion is pinned by a normative table, `docs/tables/number-to-text.tsv`, computed from the rule by an exact reference generator. `mesh-runtime` implements it.
- **The template format is specified** (`schemas/template-v1.schema.json`, `docs/manual/templates.md`): one checked component with every name resolved and no values, its compatibility rule (format version and model fingerprint only), the fingerprint's exact byte layout, and a template's canonical digest.
- **Programs and their assembly rules are specified** (`docs/manual/templates.md`): a root and a set of templates; a component is a composite exactly when the program has its template; how a composite's props bind its scope; and seven rules (one template per component, the root's template, well-formed templates of one model, sound bindings, no cycles, no composite events, no composite children), with their codes and order.
- **The render tree and the command intent are specified** (`schemas/render-v1.schema.json`, `docs/manual/runtime.md`): what a renderer draws (primitive nodes, final prop values, text runs, handler identifiers and keys, and nothing else), what a host receives from dispatch, the host's dispatch lifecycle, and the encodings of keys, handler identifiers and program identity.
- **Runtime diagnostics are specified** (`schemas/runtime-diagnostics-v1.schema.json`, `docs/manual/runtime.md`): three phases (program validation, input validation, evaluation), every new `assembly-*` and `runtime-*` code with its location, and their order.
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

[0.9.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.9.0
[0.8.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.8.0
[0.7.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.7.0
[0.6.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.6.0
[0.5.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.5.0
[0.4.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.4.0
[0.3.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.3.0
[0.2.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.2.0
[0.1.0]: https://github.com/ValanceX/Mesh/releases/tag/v0.1.0
