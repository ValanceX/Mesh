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
 * The same values give the same results as the Rust runtime.
 *
 * @packageDocumentation
 */

import {
  declaredEvents as declaredEventsWith,
  dispatch as dispatchWith,
  init as initWith,
  render as renderWith,
  Render,
} from "./engine.js";
import type {
  DeclaredEventsInput,
  DeclaredEventsResult,
  DispatchResult,
  ModuleSource,
  RenderInput,
  RenderResult,
} from "./engine.js";

export type {
  DeclaredEventsInput,
  DeclaredEventsResult,
  DispatchResult,
  ModuleSource,
  ProgramInput,
  RenderInput,
  RenderResult,
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
  RenderTree,
  RuntimeDiagnostic,
  RuntimeDiagnosticsDocument,
  RuntimeLocation,
  SourceOffset,
  SourceSpan,
  TextRun,
} from "./types.js";
export { MeshInternalError, MeshVersionError, Render } from "./engine.js";
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
