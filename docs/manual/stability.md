# API stability

What each public surface of MESH promises. The tiers have one meaning each, everywhere in the project's documentation.

| Tier | Meaning |
|---|---|
| **Stable** | In a released version (0.13.0 or earlier). Changes that break it wait for a version that says so in the [changelog](../../CHANGELOG.md); until 1.0 that can be a minor version. Diagnostic and error **codes** never change meaning and are never renamed or reused. |
| **Unreleased** | In the changelog's "Unreleased" section. Complete and tested, but not in any release: it may change before one, and the changelog will say how. |
| **Provisional** | Named so in the documentation (the constructs blocks lower to: `mesh-if`, `mesh-each`, `mesh-fragment`). Its names and exact behavior are not final, even after a release. |
| **Internal** | Not a contract. It exists because something else needs it, and may change in any release without notice. |

## By surface

| Surface | Tier |
|---|---|
| MPRX language, manifest, `template-v1`, `template-v2`, `render-v1`, `runtime-diagnostics-v1`, the CLI and its JSON output | Stable |
| `check`, `compile`, `checkProgram`, `compileProgram`, `init`, `version` of `@valancex/mesh-compiler`; `render`, `dispatch`, `declaredEvents`, `init`, `version`, `Render.tree`; `mesh_runtime::{render, dispatch, resolve}`. `render()` keeps nothing in the module, as in 0.9.0, and that is part of the contract | Stable |
| `update`, `render-patch-v1`, `render.release()`, `mesh_runtime::update`, `update_with` | Stable |
| Units: the manifest's `unit` (and `manifest-unit-clash`); `units` on `render`, `update`, `updateChanges` and `dispatch`; `Render.instances` / `Render::instances`; `instance` and `controlled` on the intent; `raise`; `mesh_runtime::{Units, Instance, render_units, update_with_units, update_changes_units, dispatch_from_units, raise, raise_from_units}`; the diagnostics `runtime-unknown-instance` and `runtime-unknown-event` | Unreleased (0.15.0): complete and tested; its first use is outside this repository (VALANCE), so it may change before it is called stable |
| `updateChanges`, `diff`, `render.version`, `[Symbol.dispose]`; `mesh_runtime::{update_changes, Changes, Change}`; the changes document; the diagnostics `runtime-changes-base-mismatch`, `runtime-invalid-change`, `runtime-changes-disagree` | Stable |
| `MeshUsageError` (including its `module-unavailable` code), `MeshUsageCode` and the `code` of the other errors, in both JavaScript packages; `RenderVersion`; the `hint` of a runtime diagnostic; `Render[Symbol.dispose]`; `mesh_runtime::Render::with_version` | Stable |
| `mesh-slot`, `mesh-fill` (named slots) and composite children; composite events (forwarding a declared event); `assembly-composite-children`; inferred composite contracts (`compileProgram` for an undeclared component, `mesh_compiler::infer`) | Stable |
| Blocks: `{#if}`, `{:else if}`, `{:else}`, `{/if}`, `{#each … as … (key)}`, `{/each}`; the diagnostics `retired-element` and `missing-repeat-key`; the retirement of `mesh-if`, `mesh-each`, `mesh-switch`, `mesh-case` and `mesh-default` as elements; the code `invalid-switch` (kept in the catalogue, no longer reported) | Stable |
| `mesh-format` (`format`, `format_edits`, `format_range_edits`, `indent_for_line`, `Options`, `Edit`, `Error`) and `mesh-lsp`'s `textDocument/formatting`, `rangeFormatting` and `onTypeFormatting`: only whitespace between tokens changes, and a source with syntax errors is not formatted (since 0.12.0) | Stable |
| The layout the formatter writes (one attribute per line in a multi-line tag, blank lines collapsed, text with content kept) | Provisional: it can be refined in a minor, within the guarantee above |
| The runtime's behavior for the constructs a block lowers to: `mesh-if`, `mesh-each` and the identity steps they add (`0x04`, `0x05`); `mesh-fragment`, its identity step (`0x07`) and `assembly-root-fragment` | Provisional |
| The module's exports (`mesh_*`) and its hook `mesh_retained_renders`; `mesh_runtime::evaluations`; the package's hidden test hooks | Internal |
| Anything under `dist/` that the package's `exports` doesn't list | Internal |

## What a host can rely on

- A document format with a `-v1` name (`render-v1`, `render-patch-v1`, `template-v1`, `runtime-diagnostics-v1`, `diagnostics-v1`) may gain **optional properties** in any release without changing its version (a new expression kind, a new value of an enumerated property, or a new required property is a new version: `template-v2` is how `has` and `??` were added, and the compiler writes it only for a template that needs it); its schema says so, and a consumer ignores properties it doesn't know (so it must not validate strictly against a copy of the schema from an older release). Removing or changing a property, or a new required one, is a new version.
- A diagnostic or error **code** keeps its meaning; match on it, never on a message. Messages, locations' wording and the order of unrelated diagnostics may improve in any release.
- Documentation examples are for the version of the page.
- A removed Stable API is deprecated first, in a release that names the replacement and how to migrate, and is removed no sooner than the next release that is allowed to break. The first release in which this applies to the Unreleased rows above is the one that includes them.
