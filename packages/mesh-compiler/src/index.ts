/**
 * The MESH compiler, in WebAssembly: check MPRX against a component
 * manifest, and get back exactly the diagnostics `mesh check --format
 * json` prints for the same inputs; or compile it, and get the template
 * `mesh compile` writes too.
 *
 * ```js
 * import { check } from "@valancex/mesh-compiler";
 *
 * const document = await check({
 *   source: generated,
 *   path: "card.mprx",
 *   model: { manifest, path: "components.json", component: "user-card" },
 * });
 * for (const diagnostic of document.diagnostics) { ... }
 * ```
 *
 * Paths are identifiers you choose; nothing reads them. The compiler runs
 * entirely in WebAssembly, and this package adds no rule of its own.
 *
 * @packageDocumentation
 */

import { check as checkWith, compile as compileWith, init as initWith } from "./engine.js";
import type { CheckInput, CompileInput, CompileResult, ModuleSource } from "./engine.js";
import type { DiagnosticsDocument } from "./document.js";

export type {
  Diagnostic,
  DiagnosticsDocument,
  Position,
  Severity,
  Span,
  Suggestion,
} from "./document.js";
export type { CheckInput, CompileInput, CompileResult, ModuleSource } from "./engine.js";
export type {
  Offset,
  Template,
  TemplateChild,
  TemplateElement,
  TemplateExpression,
  TemplateSpan,
} from "./template.js";
export { MeshInternalError, MeshVersionError } from "./engine.js";
export { version } from "./version.js";

/**
 * Checks `input.source`, as the template of `input.model.component` when a
 * model is given, and returns the diagnostics document. A problem with the
 * input is a diagnostic in the document, never an exception.
 *
 * It rejects with a `TypeError` for arguments of the wrong type, with
 * `MeshVersionError` if the WebAssembly module isn't this version's, and
 * with `MeshInternalError` if the compiler itself fails. In a browser,
 * call {@link init} first.
 */
export function check(input: CheckInput): Promise<DiagnosticsDocument> {
  return checkWith(input);
}

/**
 * Checks `input.source` as the template of `input.model.component`, as
 * {@link check} does, and, only when that finds no error, compiles it to
 * a template (`template-v1`): exactly what `mesh compile` reports and
 * writes for the same inputs. Warnings don't stop it. A model is
 * required, since a template is always a component's.
 *
 * `diagnostics` is the document `check` returns; `template` is absent
 * when it has an error. It rejects as `check` does.
 */
export function compile(input: CompileInput): Promise<CompileResult> {
  return compileWith(input);
}

/**
 * Loads the WebAssembly module from `source`: a URL (or a string resolved
 * against the page), its bytes, or a compiled `WebAssembly.Module`, and
 * checks its version. Needed once in a browser, before the first check;
 * in Node the package loads its own module. The module is
 * `@valancex/mesh-compiler/mesh.wasm`.
 */
export function init(source: ModuleSource): Promise<void> {
  return initWith(source);
}
