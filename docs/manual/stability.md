# API stability

What each public surface of MESH promises. The tiers have one meaning each, everywhere in the project's documentation.

| Tier | Meaning |
|---|---|
| **Stable** | In a released version (0.9.0 or earlier). Changes that break it wait for a version that says so in the [changelog](../../CHANGELOG.md); until 1.0 that can be a minor version. Diagnostic and error **codes** never change meaning and are never renamed or reused. |
| **Unreleased** | In the changelog's "Unreleased" section. Complete and tested, but not in any release: it may change before one, and the changelog will say how. |
| **Provisional** | Named so in the documentation (`mesh-if`, `mesh-each`). Its names and exact behavior are not final, even after a release. |
| **Internal** | Not a contract. It exists because something else needs it, and may change in any release without notice. |

## By surface

| Surface | Tier |
|---|---|
| MPRX language, manifest, `template-v1`, `render-v1`, `runtime-diagnostics-v1`, the CLI and its JSON output | Stable |
| `render`, `dispatch`, `declaredEvents`, `init`, `version`, `Render.tree`; `mesh_runtime::{render, dispatch, resolve}`. `render()` keeps nothing in the module, as in 0.9.0, and that is part of the contract | Stable |
| `update`, `render-patch-v1`, `render.release()`, `mesh_runtime::update`, `update_with` | Unreleased |
| `updateChanges`, `diff`, `render.version`, `[Symbol.dispose]`; `mesh_runtime::{update_changes, Changes, Change}`; the changes document; the diagnostics `runtime-changes-base-mismatch`, `runtime-invalid-change`, `runtime-changes-disagree` | Unreleased |
| `MeshUsageError`, `MeshUsageCode` and the `code` of the other errors | Unreleased |
| `mesh-slot` and composite children; composite events (forwarding a declared event); `assembly-composite-children` | Unreleased |
| `mesh-if`, `mesh-each` and the identity steps they add (`0x04`, `0x05`) | Provisional |
| The module's exports (`mesh_*`) and its hook `mesh_retained_renders`; `mesh_runtime::evaluations`; the package's hidden test hooks | Internal |
| Anything under `dist/` that the package's `exports` doesn't list | Internal |

## What a host can rely on

- A document format with a `-v1` name (`render-v1`, `render-patch-v1`, `template-v1`, `runtime-diagnostics-v1`, `diagnostics-v1`) may gain **optional properties** in any release without changing its version; its schema says so, and a consumer ignores properties it doesn't know (so it must not validate strictly against a copy of the schema from an older release). Removing or changing a property, or a new required one, is a new version.
- A diagnostic or error **code** keeps its meaning; match on it, never on a message. Messages, locations' wording and the order of unrelated diagnostics may improve in any release.
- Documentation examples are for the version of the page.
- A removed Stable API is deprecated first, in a release that names the replacement and how to migrate, and is removed no sooner than the next release that is allowed to break. The first release in which this applies to the Unreleased rows above is the one that includes them.
