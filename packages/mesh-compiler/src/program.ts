/**
 * Compiling a program: the components' templates, and the program's parts.
 *
 * A program has no document of its own (spec §9, "Programs"): a host gives the
 * compiler's program check and the runtime its parts, the root's name and the
 * templates' texts, with the manifest's text. This composes the operations
 * that already exist, in the one order a host needs them: `compile` for each
 * component, then `checkProgram` for the templates it made. It adds no rule
 * and no check of its own, and it reads nothing but its input.
 *
 * Not API on its own: `compileProgram` is the package's.
 */

import type { DiagnosticsDocument } from "./document.js";
import { checkProgram, compile } from "./engine.js";
import type { ProgramInput } from "./engine.js";
import type { RuntimeDiagnosticsDocument } from "./runtime-document.js";

/** One component's MPRX, to be compiled into the program as that component's template. */
export interface ProgramComponent {
  /** The component whose template the source is. Always explicit. */
  component: string;
  /** The MPRX source text. */
  source: string;
  /** An opaque identifier for the source, used only to name it in diagnostics. Never read. */
  path: string;
}

/** One program compile's inputs. */
export interface CompileProgramInput {
  /** The model: the manifest every template is compiled against, which is also the program's own. */
  model: {
    /** The manifest's text. */
    manifest: string;
    /** An opaque identifier for the manifest, used only to name it in diagnostics. Never read. */
    path: string;
  };
  /** The root component: the one the program renders. */
  root: string;
  /**
   * The components that have a template in the program, in order. A component that is not listed has no template, and so is a primitive
   * (spec §9, "Composite or primitive"): nothing here finds components, so a composite's template is listed or it is absent.
   */
  components: readonly ProgramComponent[];
}

/** What a program compile gives. */
export interface CompileProgramResult {
  /** What `compile` returned for each component's source, in the order of the input's `components`: its diagnostics document. */
  components: ReadonlyArray<{ component: string; diagnostics: DiagnosticsDocument }>;
  /**
   * What `checkProgram` returned for the templates, as it would for the same parts. Present exactly when no component had an error, since only
   * then are there templates to check.
   */
  assembly?: RuntimeDiagnosticsDocument;
  /**
   * The program, as `checkProgram` and the runtime take it: the model's text, the root, and the templates as text, in the order of the input's
   * `components`. Present exactly when no component had an error and the program check found none.
   */
  program?: ProgramInput;
}

function validate(input: CompileProgramInput): void {
  if (typeof input !== "object" || input === null) {
    throw new TypeError("compileProgram() takes an object: { model, root, components }");
  }
  const model = input.model;
  if (typeof model !== "object" || model === null) {
    throw new TypeError("model must be an object: { manifest, path }");
  }
  const strings: [string, unknown][] = [
    ["model.manifest", model.manifest],
    ["model.path", model.path],
    ["root", input.root],
  ];
  if (!Array.isArray(input.components)) {
    throw new TypeError("components must be an array of { component, source, path }");
  }
  input.components.forEach((entry: unknown, index) => {
    if (typeof entry !== "object" || entry === null) {
      throw new TypeError(`components[${index}] must be an object: { component, source, path }`);
    }
    const written = entry as Record<string, unknown>;
    strings.push(
      [`components[${index}].component`, written["component"]],
      [`components[${index}].source`, written["source"]],
      [`components[${index}].path`, written["path"]],
    );
  });
  for (const [name, value] of strings) {
    if (typeof value !== "string") {
      throw new TypeError(`${name} must be a string`);
    }
  }
}

/** Compiles a program. See the package's `compileProgram`. */
export async function compileProgram(input: CompileProgramInput): Promise<CompileProgramResult> {
  validate(input);
  const { manifest, path } = input.model;
  const compiled = [];
  // Every component is compiled, so that one run reports every component's errors, not the first's.
  for (const entry of input.components) {
    compiled.push(await compile({ source: entry.source, path: entry.path, model: { manifest, path, component: entry.component } }));
  }
  const components = compiled.map((result, index) => ({
    component: input.components[index]!.component,
    diagnostics: result.diagnostics,
  }));
  const templates: string[] = [];
  for (const result of compiled) {
    if (result.template === undefined) {
      return { components };
    }
    // The text the runtime and the program check take: the template as the compiler returned it, written out as JSON.
    templates.push(JSON.stringify(result.template));
  }
  const parts: ProgramInput = { model: manifest, root: input.root, templates };
  const assembly = await checkProgram(parts);

  return assembly.diagnostics.length === 0 ? { components, assembly, program: parts } : { components, assembly };
}
