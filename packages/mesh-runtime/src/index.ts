/**
 * The MESH runtime, in WebAssembly: render a program of MESH templates
 * against a snapshot, for a renderer to draw, and turn the events it
 * reports into command intents, for the host.
 *
 * ```js
 * import { render, dispatch } from "@valancex/mesh-runtime";
 *
 * const result = await render({ program: { root: "users", templates }, model, snapshot });
 * if (result.diagnostics) throw new Error(result.diagnostics.diagnostics[0].message);
 * draw(result.render.tree);
 * // later, when the renderer reports an event on that tree:
 * const { intent } = await dispatch(result.render, handler, payload);
 * ```
 *
 * The runtime runs entirely in WebAssembly: this package encodes values
 * and transports them, and evaluates, converts and judges nothing (I11).
 * The one thing it does with a tree is apply the runtime's own patches to
 * the previous one, in `patches.ts`: it moves parts by key, and reads no value.
 * The same values give the same results as the Rust runtime.
 *
 * @packageDocumentation
 */

import {
  declaredEvents as declaredEventsWith,
  dispatch as dispatchWith,
  init as initWith,
  render as renderWith,
  update as updateWith,
  updateChanges as updateChangesWith,
  Render,
} from "./engine.js";
import type {
  Changes,
  DeclaredEventsInput,
  DeclaredEventsResult,
  DispatchResult,
  ModuleSource,
  RenderInput,
  RenderResult,
  UpdateResult,
} from "./engine.js";

export type {
  Change,
  Changes,
  PathStep,
  RenderVersion,
  DeclaredEventsInput,
  DeclaredEventsResult,
  DispatchResult,
  ModuleSource,
  ProgramInput,
  RenderInput,
  RenderResult,
  UpdateResult,
} from "./engine.js";
export type {
  BoundaryValue,
  CommandIntent,
  DeclaredEvent,
  Host,
  IntentArgument,
  ModelPosition,
  ModelSpan,
  RenderNode,
  RenderPatch,
  RenderPatches,
  RenderTree,
  RuntimeDiagnostic,
  RuntimeDiagnosticsDocument,
  RuntimeLocation,
  SourceOffset,
  SourceSpan,
  TextRun,
} from "./types.js";
export { MeshInternalError, MeshUsageError, MeshVersionError, Render } from "./engine.js";
export type { MeshUsageCode } from "./engine.js";
export { diff } from "./changes.js";
export { version } from "./version.js";

/**
 * Renders `input.program` against `input.model` and `input.snapshot`:
 * validates the program, then the snapshot, then evaluates. Returns
 * `{ render }`, whose `tree` a renderer draws and which dispatch needs
 * later, or `{ diagnostics }`, the runtime diagnostics document. A
 * problem with the inputs is a diagnostic, never an exception.
 *
 * The snapshot is encoded when this is called, so changing the host's
 * objects afterwards changes nothing about this render. It rejects with
 * a `TypeError` for arguments of the wrong type, or a snapshot that
 * isn't a plain object; with `MeshVersionError` if the WebAssembly
 * module isn't this version's; and with `MeshInternalError` if the
 * runtime itself fails. In a browser, call {@link init} first.
 */
export function render(input: RenderInput): Promise<RenderResult> {
  return renderWith(input);
}

/**
 * Updates `previous` to a new `snapshot` of the same program: returns
 * `{ render, patches }`, the new render and the `render-patch-v1`
 * patches that turn `previous.tree` into `render.tree` (a renderer applies
 * them in order, and never compares lists), or `{ diagnostics }`, in which
 * case `previous` is untouched and still dispatches. A change of structure
 * the patch format can't yet express gives one `replace` of the whole tree.
 *
 * **The module keeps the render this returns** (`render()` keeps nothing), so
 * the next `update` from it reuses what the module already computed, and costs
 * what the same update costs in Rust. `previous` is kept too, if it was made
 * by `update`, and stays valid until released. Call `release()` on a render
 * `update` made once it is no longer the one to update from or dispatch with
 * (typically: on `previous`, after its patches are applied). One that is never
 * released is released when it is garbage collected, but that can be late, and
 * until then its copy is memory in the module. A render the module doesn't
 * hold (one `render()` made, or one released) is derived again by the update,
 * which then costs a render more.
 *
 * It rejects as {@link render} does.
 */
export function update(previous: Render, snapshot: Record<string, unknown>): Promise<UpdateResult> {
  return updateWith(previous, snapshot);
}

/**
 * Updates `previous` by **changes** to its snapshot, not a whole new snapshot:
 * returns `{ render, patches }` as {@link update} does, or `{ diagnostics }`,
 * in which case `previous` is untouched and still valid.
 *
 * `changes` is `{ base, changes: [{ op, path, value? }] }`. `base` is the
 * `version` of the render the changes were computed against, which must be
 * `previous`'s: changes are only ever applied to the render they name, so one
 * computed against another state, applied twice, or applied out of order is
 * refused (`runtime-changes-base-mismatch`). Each edit is a `set`, an `insert`
 * (into a list, shifting the rest) or a `remove`, at a `path` of record field
 * names and list indices from a scope name down; edits apply in order, each
 * seeing those before it. The changes *define* the new snapshot: the runtime
 * applies them to the one it holds, sharing what they don't touch, and
 * validates only the values they give. So an update costs the changes and what
 * depends on them, not the size of the snapshot, and nothing of the snapshot
 * crosses the boundary.
 *
 * `previous` may be any render: if the module holds it, the changes apply to
 * it there; if not (`render()` made it, or it was released), the module
 * derives it from its own snapshot first, under the version it was given, so
 * the first `updateChanges` from a plain render costs a render more. A render
 * that `updateChanges` itself made has no snapshot outside the module, so once
 * it is released (or the module is replaced) it can't be updated or
 * dispatched with: this rejects with a `MeshUsageError` (`render-gone`) for
 * one whose copy is gone, and the host renders again from a whole snapshot.
 *
 * With `options.verify`, the whole snapshot the host believes it now has, the
 * changes are checked against it (`runtime-changes-disagree`, at the first
 * path that differs): a check of whoever computed the changes, at the cost of
 * a validation of the whole snapshot, for tests and development. {@link diff}
 * computes changes from two snapshots.
 *
 * It rejects as {@link render} does.
 */
export function updateChanges(
  previous: Render,
  changes: Changes,
  options?: { readonly verify?: Record<string, unknown> },
): Promise<UpdateResult> {
  return updateChangesWith(previous, changes, options);
}

/**
 * Dispatches an event a renderer reported on `render`'s tree: its
 * handler identifier, and its payload (absent when not given, or
 * `undefined`). Returns `{ intent }`, the command intent, or
 * `{ diagnostics }`. It evaluates against the snapshot `render` was made
 * from, never a newer one, so pass the render whose tree the renderer
 * drew. It validates everything again.
 *
 * It rejects with a `TypeError` for a `render` that {@link render}
 * didn't return, or a handler that isn't a string, and otherwise as
 * {@link render} does.
 */
export function dispatch(render: Render, handler: string, payload?: unknown): Promise<DispatchResult> {
  return dispatchWith(render, handler, payload);
}

/**
 * The events the templates of `input.program` declare, against
 * `input.model`: validates the program as {@link render} does, then
 * returns `{ events }`, each one's declaring component, event, command
 * and source span, or `{ diagnostics }`, the same diagnostics render
 * would give for that program. The events are the program's own, not a
 * render's: those inside composites, inactive conditionals and repeated
 * sections are there, and nothing is deduplicated. It needs no snapshot.
 *
 * It rejects with a `TypeError` for arguments of the wrong type, and
 * otherwise as {@link render} does.
 */
export function declaredEvents(input: DeclaredEventsInput): Promise<DeclaredEventsResult> {
  return declaredEventsWith(input);
}

/**
 * Loads the WebAssembly module from `source`: a URL (or a string resolved
 * against the page), its bytes, or a compiled `WebAssembly.Module`, and
 * checks its version. Optional: without it the package loads its own
 * module (`mesh-runtime.wasm`, next to its code) on the first call, in Node
 * and in a browser. Call it to load another copy, or from another URL, which
 * a bundler may need: the module is
 * `@valancex/mesh-runtime/mesh-runtime.wasm`.
 */
export function init(source: ModuleSource): Promise<void> {
  return initWith(source);
}
