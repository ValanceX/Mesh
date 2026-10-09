/**
 * The MESH compiler, in WebAssembly: check MPRX against a component
 * manifest, and get back exactly the diagnostics `mesh check --format
 * json` prints for the same inputs; compile it, and get the template
 * `mesh compile` writes too; check a program of templates, as `mesh
 * check-program` does; or compile a whole program's components, and get
 * the program's parts for the runtime.
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

import {
  check as checkWith,
  checkProgram as checkProgramWith,
  compile as compileWith,
  init as initWith,
} from "./engine.js";
import type { CheckInput, CompileInput, CompileResult, ModuleSource, ProgramInput } from "./engine.js";
import type { DiagnosticsDocument } from "./document.js";
import { compileProgram as compileProgramWith } from "./program.js";
import type { CompileProgramInput, CompileProgramResult } from "./program.js";
import type { RuntimeDiagnosticsDocument } from "./runtime-document.js";

export type {
  Diagnostic,
  DiagnosticsDocument,
  Position,
  Severity,
  Span,
  Suggestion,
} from "./document.js";
export type { CheckInput, CompileInput, CompileResult, ModuleSource, ProgramInput } from "./engine.js";
export type { CompileProgramInput, CompileProgramResult, ProgramComponent } from "./program.js";
export type {
  ModelPosition,
  ModelSpan,
  RuntimeDiagnostic,
  RuntimeDiagnosticsDocument,
  RuntimeLocation,
  SourceOffset,
  SourceSpan,
} from "./runtime-document.js";
export type {
  Offset,
  Template,
  TemplateChild,
  TemplateElement,
  TemplateExpression,
  TemplateSpan,
} from "./template.js";
export { MeshInternalError, MeshUsageError, MeshVersionError } from "./engine.js";
export type { MeshUsageCode } from "./engine.js";
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
 * Checks a program: the manifest `input.model`, then each of
 * `input.templates` (well-formed, of a format version this MESH reads,
 * and compiled against this model), then the assembly rules, with
 * `input.root` as the root component. Returns the runtime diagnostics
 * document, which is empty when the program is valid: exactly what
 * `mesh check-program --format json` prints, and what the runtime's
 * render reports for the same program before it looks at a snapshot.
 * It produces nothing else. It rejects as `check` does.
 */
export function checkProgram(input: ProgramInput): Promise<RuntimeDiagnosticsDocument> {
  return checkProgramWith(input);
}

/**
 * Compiles a program's components against one manifest, checks the program
 * they make, and returns the program's parts: the input `checkProgram` and
 * the runtime's `render` take, with the templates as text, so a host passes
 * the result on as it is.
 *
 * Each of `input.components` is compiled as {@link compile} compiles it, and
 * `components` in the result holds each one's diagnostics document, in
 * order, whether it has errors or not (warnings don't stop a template). Only
 * when none has an error are the templates checked as a program, by
 * {@link checkProgram}, whose document is `assembly`. `program` is present
 * exactly when neither found an error. This adds no rule of its own, and
 * needs no module but the compiler's: in a browser, call {@link init} first,
 * as for any check.
 *
 * It takes the sources it is given and finds none: a component that isn't
 * listed has no template, and is a primitive. It rejects as `check` does.
 */
export function compileProgram(input: CompileProgramInput): Promise<CompileProgramResult> {
  return compileProgramWith(input);
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
