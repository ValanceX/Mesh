# Baseline for the ecosystem API design review

> A record of the freeze as it was. Since then `render({ keep })` was removed (every render is kept; see [the review](./2026-10-09-api-design-review.md), M6, and its second pass for a problem with that), and the review's other changes are listed there.

Status: **frozen for review** (2026-10-09). No feature work continues on the `ccr-969a5158-vobqhh` branches until the review's principles are applied or set aside. This file records what was decided, what is built, and what the review covers, so the review starts from a fixed point.

## Decision

Feature work stops at the changes form. The next piece of work is a review of every public API in the ecosystem against a set of API design principles that have not been given yet. The principles will be applied where they fit, and each place they don't is recorded with the reason. Nothing is renamed, reshaped or removed before the principles are known.

## The frozen state

| Repository | Branch | Commit |
|---|---|---|
| Mesh | `ccr-969a5158-vobqhh` | the commit that adds this file (its parent is `fd5c53f`, "Changes form") |
| Port | `ccr-969a5158-vobqhh` | `1c92af9`, "M2: patch() gains insert, remove and move" (unchanged since, so its freeze note is a document only) |
| Nexus, Valance | | untouched by this work |

Checks at the freeze: Mesh `cargo clippy --workspace --all-targets` and `cargo test --workspace` clean; `@valancex/mesh-runtime` 91 and `@valancex/mesh-compiler` 46 JavaScript tests passing; Port 311 tests passing. The browser tests (`browser.test.mjs`) hang in the cloud container and have not run. Nothing is released: all of it is "Unreleased" in the changelogs.

## What was built, and the decisions that shaped it

Built, in order: static read sets and `update` with `render-patch-v1` (props and text); Port's `patch`; the render retained in the module; structural patches (`insert`, `remove`, `move`) in Mesh and Port; an update that costs what changed (shared tree, per-node memos, repeat shortcut); the module returning patches only; composite children (`mesh-slot`); composite events (forwarding a declared event); the changes form (`update_changes` / `updateChanges`, `diff`, versioned bases, verify mode).

Decisions, each also recorded where it lives:

1. **Fine-grained reactivity by dependency-tracked patches,** with the equivalence law (patches applied to the previous tree equal a full render) as the test. Design: [fine-grained reactivity and composition](./2026-10-06-mesh-fine-grained-reactivity-and-composition.md).
2. **No mandatory immutable-data library and no Effect dependency in the runtime.** Sharing is by reference (`Rc`, frozen host values), equality is the boundary's own (numbers by bits).
3. **The module keeps renders between calls** and the host releases them (`render.release()`): the module is no longer stateless.
4. **The module returns patches, not trees;** the JavaScript package applies them (`patches.ts`), a named exception to the rule that the package evaluates nothing (I11).
5. **Composite children and events add no syntax** and no manifest change: a template's `mesh-slot` decides whether children are accepted; a declared event is forwarded by naming it as a handler.
6. **The host hands over changes** (`set`, `insert`, `remove` at paths). Deviations from the first design: `base` is a render **version**, not a snapshot digest; `insert` was added; a render made from changes keeps its snapshot only in the module; `diff` lives in the JavaScript package as a host-side utility (the second named I11 exception).

## Public API surface to be reviewed

**Mesh, Rust.** `mesh_runtime`: `render`, `update`, `update_with`, `update_changes`, `dispatch`, `dispatch_from`, `resolve`, `diff`, `patches_to_json`, `to_json`, `declared_events_to_json`, `number_to_text`, `Render` (`version()`, `host_snapshot()`), `Update`, `Changes`, `Change`, `Program`, `Tree`, `Node`, `TreeChild`, `Intent`, `HostValue`, `HostRecord`, `HostKey`, `RuntimeCode`, `RuntimeDiagnostic`, `Location`, `PathSegment`, `Form`. Other crates: `mesh-parser`, `mesh-syntax`, `mesh-semantic`, `mesh-analysis`, `mesh-manifest`, `mesh-template`, `mesh-compiler`, `mesh-lsp`, `mesh-wasm`, `mesh-runtime-wasm`, and the `mesh` CLI (commands, flags, JSON output, exit statuses).

**Mesh, JavaScript.** `@valancex/mesh-runtime`: `render` (with `keep`), `dispatch`, `update`, `updateChanges`, `diff`, `declaredEvents`, `init`, `version`, `Render` (`tree`, `version`, `release()`), `MeshInternalError`, `MeshVersionError`. `@valancex/mesh-compiler`: `check`, `compile`, `checkProgram`, `compileProgram`, `init`, `version`, the same two error classes. `mesh-lsp` packaging.

**Mesh, documents that are API.** `render-v1`, `render-patch-v1`, `template-v1` and the runtime diagnostics schemas under `schemas/`; the manifest format; the diagnostic codes (reference in `docs/manual/diagnostics.md`; codes are stable and never renamed); the changes document `{ base, changes: [{ op, path, value? }] }`.

**Port.** `@valancex/port-web`: `createWebPort` and its `WebPort` (`draw`, `update`, `patch`, `unmount`, `hydrate`, `inspect`), `WebPortOptions`, the primitives table helpers (`attribute`, `booleanAttribute`, `controlled`, `property`, `textProperty`), `WebRealizationError` and its codes, `UNKNOWN_COMPONENT_ELEMENT`; and `docs/CONTRACT.md` (contract version 1, to which `patch` was added).

**Nexus and Valance.** Their public surfaces, to be read as part of the review; neither was changed here.

## Known tensions to take to the review

These are places where the current API already shows a choice the principles may judge; they are listed so nothing is rediscovered, not as conclusions.

- `update` and `updateChanges` are two verbs for one operation with two inputs; `render` has an option (`keep`) that changes whether later calls are allowed.
- A `Render` has hidden state in the module (`release()`); some operations on it throw a `TypeError` once released or once the module was replaced, where most problems are returned as diagnostics.
- `updateChanges` takes the version as `base` from the caller; the version is a counter, not a content address.
- `diff` is exported from the runtime package although nothing in the runtime uses it.
- Errors are three kinds: diagnostics documents (returned), `TypeError` for wrong argument types, `MeshInternalError` / `MeshVersionError`.
- Rust and JavaScript names differ where they need not (`update_changes` / `updateChanges` is by convention; `host_snapshot` has no JavaScript counterpart).
- `mesh_render_kept`, `mesh_update_changes`, `mesh_dispatch_kept`, `mesh_update` and `mesh_release` are module exports that the package depends on and hosts must not.

## Not built, kept out of the review's way

Named slots; a table from a changed value to the nodes that read it (the one remaining cost is a visit per item of a changed repeat); Valance's use of `update` / changes (M6); session-log replay and time travel; Port's Chromium tests; moving the end-to-end Port check into Port's `integration/` slice once Mesh is released.

## How the review will run

1. The principles are given.
2. Each public surface above is read against them, and findings are recorded per surface with the principle, the evidence (file and line, or the doc section), and a recommendation.
3. Changes the principles clearly require are made, in small commits, with docs and changelogs updated in the same commit; changes that are a matter of judgment are put to the owner first.
4. Anything that would break a released API (0.9.0 and earlier) is flagged and not made silently; unreleased surface may change.
