/**
 * The internal engine behind the public API: loading the WebAssembly
 * module, checking its version, keeping one instance, moving bytes in and
 * out of it, and discarding it after a failure.
 *
 * Deliberately boring (outline v0.4 I6): nothing here knows MPRX, the
 * manifest, positions, diagnostics or templates. It transfers UTF-8 in
 * and the result out (a diagnostics document, or for a compile, that
 * document and a template), and checks only that what came out has that
 * shape at all. Not public API (D11): the package's `exports` map
 * doesn't expose this file.
 */

import type { DiagnosticsDocument } from "./document.js";
import type { RuntimeDiagnosticsDocument } from "./runtime-document.js";
import type { Template } from "./template.js";
import { version } from "./version.js";

/**
 * A failure at the boundary with the compiler: a trap (a Rust panic, or
 * running out of memory), or a result that isn't a diagnostics document.
 * It is never how a check reports a problem with its input; those are
 * diagnostics. The failed compiler instance is discarded, and the next
 * check uses a fresh one.
 */
export class MeshInternalError extends Error {
  override name = "MeshInternalError";
}

/**
 * The WebAssembly module isn't the version this package was built for, or
 * isn't a MESH module at all. No check runs against it.
 */
export class MeshVersionError extends Error {
  override name = "MeshVersionError";
}

/** What {@link init} accepts: where the module is, its bytes, or the module. */
export type ModuleSource = URL | string | BufferSource | WebAssembly.Module;

/** One check's inputs. */
export interface CheckInput {
  /** The MPRX source text. */
  source: string;
  /** An opaque identifier for the source, used only to name it in diagnostics. Never read. */
  path: string;
  /** The model to check against. Without one, only syntax and structure are checked. */
  model?: {
    /** The manifest's text. */
    manifest: string;
    /** An opaque identifier for the manifest, used only to name it in diagnostics. Never read. */
    path: string;
    /** The component whose template the source is. Always explicit. */
    component: string;
  };
}

/** One compile's inputs: a check's, with the model required. */
export interface CompileInput {
  /** The MPRX source text. */
  source: string;
  /** An opaque identifier for the source, used only to name it in diagnostics. Never read. */
  path: string;
  /** The model to check against: a template is always a component's. */
  model: {
    /** The manifest's text. */
    manifest: string;
    /** An opaque identifier for the manifest, used only to name it in diagnostics. Never read. */
    path: string;
    /** The component whose template the source is. Always explicit. */
    component: string;
  };
}

/**
 * What a compile gives: the diagnostics document `check` would return for
 * the same inputs, and, only when it has no error, the template.
 */
export interface CompileResult {
  diagnostics: DiagnosticsDocument;
  /** The `template-v1` document. Absent when the diagnostics have an error. */
  template?: Template;
}

/** One program check's inputs. */
export interface ProgramInput {
  /** The manifest's text: the model the templates were compiled against. */
  model: string;
  /** The root component: the one the program renders. */
  root: string;
  /** The program's templates (`template-v1` documents, as text), in order. */
  templates: readonly string[];
}

/** The module's exports this engine uses. They're internal to the package. */
interface Exports {
  memory: WebAssembly.Memory;
  mesh_version_ptr(): number;
  mesh_version_len(): number;
  mesh_alloc(len: number): number;
  mesh_free(ptr: number, len: number): void;
  mesh_check(...args: number[]): number;
  mesh_compile(...args: number[]): number;
  mesh_check_program(...args: number[]): number;
  mesh_result_ptr(): number;
  mesh_result_len(): number;
  mesh_result_clear(): void;
}

const CHECK_EXPORTS = [
  "mesh_alloc",
  "mesh_free",
  "mesh_check",
  "mesh_compile",
  "mesh_check_program",
  "mesh_result_ptr",
  "mesh_result_len",
  "mesh_result_clear",
] as const;

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/** The compiled module, once loaded. */
let compiled: WebAssembly.Module | undefined;
/** The instance in use. Dropped after any boundary failure. */
let instance: Exports | undefined;
/** Checks run one at a time, in call order. */
let queue: Promise<unknown> = Promise.resolve();

function isNode(): boolean {
  return typeof process !== "undefined" && typeof process.versions?.node === "string";
}

async function bytesOf(location: URL): Promise<BufferSource> {
  if (location.protocol === "file:" && isNode()) {
    const { readFile } = await import("node:fs/promises");
    return readFile(location);
  }
  const response = await fetch(location);
  if (!response.ok) {
    throw new Error(`could not fetch the MESH module from ${location.href}: ${response.status}`);
  }
  return response.arrayBuffer();
}

async function compileModule(source: ModuleSource): Promise<WebAssembly.Module> {
  if (source instanceof WebAssembly.Module) {
    return source;
  }
  if (typeof source === "string" || source instanceof URL) {
    const base = typeof location === "undefined" ? undefined : location.href;
    return WebAssembly.compile(await bytesOf(new URL(source, base)));
  }
  return WebAssembly.compile(source);
}

/** Instantiates `module` and checks it's a MESH module of this version (I10). */
async function instantiate(module: WebAssembly.Module): Promise<Exports> {
  let exports: Record<string, unknown>;
  try {
    exports = (await WebAssembly.instantiate(module, {})).exports;
  } catch (cause) {
    throw new MeshVersionError("this WebAssembly module isn't a MESH module", { cause });
  }
  const { memory, mesh_version_ptr, mesh_version_len } = exports;
  if (
    !(memory instanceof WebAssembly.Memory) ||
    typeof mesh_version_ptr !== "function" ||
    typeof mesh_version_len !== "function"
  ) {
    throw new MeshVersionError("this WebAssembly module isn't a MESH module: it has no version");
  }
  let found: string;
  try {
    const ptr = mesh_version_ptr() as number;
    const len = mesh_version_len() as number;
    found = decoder.decode(new Uint8Array(memory.buffer, ptr, len));
  } catch (cause) {
    throw new MeshVersionError("this WebAssembly module's version can't be read", { cause });
  }
  if (found !== version) {
    throw new MeshVersionError(
      `this package is @valancex/mesh-compiler ${version}, but its WebAssembly module is ${found}`,
    );
  }
  for (const name of CHECK_EXPORTS) {
    if (typeof exports[name] !== "function") {
      throw new MeshVersionError(`this WebAssembly module ${found} has no ${name}`);
    }
  }
  // A plain copy of the exports (theirs is frozen), which the package's
  // failure tests can instrument.
  return { ...exports } as unknown as Exports;
}

/**
 * Loads the module from `source` and checks its version. In Node, the
 * package loads its own module on the first check, so this is only
 * needed to use another copy; in a browser, it's needed once, before the
 * first check. If it fails, nothing is kept.
 */
export function init(source: ModuleSource): Promise<void> {
  const run = queue.then(async () => {
    compiled = undefined;
    instance = undefined;
    const module = await compileModule(source);
    instance = await instantiate(module);
    compiled = module;
  });
  queue = run.catch(() => undefined);
  return run;
}

async function current(): Promise<Exports> {
  if (instance) {
    return instance;
  }
  if (!compiled) {
    if (!isNode()) {
      throw new Error(
        "@valancex/mesh-compiler: call init() with the URL of mesh.wasm before the first check",
      );
    }
    const module = await compileModule(new URL("./mesh.wasm", import.meta.url));
    instance = await instantiate(module);
    compiled = module;
    return instance;
  }
  instance = await instantiate(compiled);
  return instance;
}

/** Copies `bytes` into a new buffer in the module; returns its address. */
function put(exports: Exports, bytes: Uint8Array): number {
  const ptr = exports.mesh_alloc(bytes.length);
  // Read `memory.buffer` after allocating: growing memory replaces it.
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  return ptr;
}

function isDocument(value: unknown): value is DiagnosticsDocument {
  return (
    typeof value === "object" &&
    value !== null &&
    "version" in value &&
    Array.isArray((value as { diagnostics?: unknown }).diagnostics)
  );
}

function isCompileResult(value: unknown): value is { diagnostics: unknown; template: unknown } {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const { diagnostics, template } = value as { diagnostics?: unknown; template?: unknown };
  return isDocument(diagnostics) && (template === null || (typeof template === "object" && template !== undefined));
}

/**
 * One call on `exports`: copies `inputs` into the module, calls `call`
 * with each one's address and length, frees them, and returns the
 * result, parsed. Any exception means the instance failed.
 */
function transfer(
  exports: Exports,
  inputs: readonly Uint8Array[],
  call: (pointers: number[]) => number,
): unknown {
  const buffers = inputs.map((bytes) => ({ ptr: put(exports, bytes), len: bytes.length }));
  const status = call(buffers.flatMap(({ ptr, len }) => [ptr, len]));
  for (const { ptr, len } of buffers) {
    exports.mesh_free(ptr, len);
  }
  if (status !== 0) {
    throw new MeshInternalError(`the compiler refused its input (status ${status})`);
  }
  const bytes = new Uint8Array(
    exports.memory.buffer,
    exports.mesh_result_ptr(),
    exports.mesh_result_len(),
  ).slice();
  exports.mesh_result_clear();
  return JSON.parse(decoder.decode(bytes));
}

/** A check's or compile's five texts, as the module takes them. */
function checkInputs(input: CheckInput): Uint8Array[] {
  const model = input.model;
  return [
    input.source,
    input.path,
    model?.manifest ?? "",
    model?.path ?? "",
    model?.component ?? "",
  ].map((text) => encoder.encode(text));
}

/**
 * A text list, as the module reads one: a u32 count, then each text as a
 * u32 byte length and its UTF-8 bytes, little-endian.
 */
function textList(texts: readonly string[]): Uint8Array {
  const encoded = texts.map((text) => encoder.encode(text));
  const size = 4 + encoded.reduce((sum, bytes) => sum + 4 + bytes.length, 0);
  const list = new Uint8Array(size);
  const view = new DataView(list.buffer);
  view.setUint32(0, encoded.length, true);
  let at = 4;
  for (const bytes of encoded) {
    view.setUint32(at, bytes.length, true);
    list.set(bytes, at + 4);
    at += 4 + bytes.length;
  }
  return list;
}

function checkResult(value: unknown): DiagnosticsDocument {
  if (!isDocument(value)) {
    throw new MeshInternalError("the compiler's result isn't a diagnostics document");
  }
  return value;
}

function programResult(value: unknown): RuntimeDiagnosticsDocument {
  if (!isDocument(value)) {
    throw new MeshInternalError("the compiler's result isn't a runtime diagnostics document");
  }
  return value as unknown as RuntimeDiagnosticsDocument;
}

function compileResult(value: unknown): CompileResult {
  if (!isCompileResult(value)) {
    throw new MeshInternalError("the compiler's result isn't a compile result");
  }
  const diagnostics = value.diagnostics as DiagnosticsDocument;
  return value.template === null
    ? { diagnostics }
    : { diagnostics, template: value.template as Template };
}

function validate(input: CheckInput, compiling: boolean): void {
  const what = compiling ? "compile() takes an object: { source, path, model }" : "check() takes an object: { source, path, model? }";
  if (typeof input !== "object" || input === null) {
    throw new TypeError(what);
  }
  const strings: [string, unknown][] = [
    ["source", input.source],
    ["path", input.path],
  ];
  if (compiling && input.model === undefined) {
    throw new TypeError("compile() needs a model: { manifest, path, component }");
  }
  if (input.model !== undefined) {
    const model = input.model;
    if (typeof model !== "object" || model === null) {
      throw new TypeError("model must be an object: { manifest, path, component }");
    }
    strings.push(
      ["model.manifest", model.manifest],
      ["model.path", model.path],
      ["model.component", model.component],
    );
  }
  for (const [name, value] of strings) {
    if (typeof value !== "string") {
      throw new TypeError(`${name} must be a string`);
    }
  }
}

function validateProgram(input: ProgramInput): void {
  if (typeof input !== "object" || input === null) {
    throw new TypeError("checkProgram() takes an object: { model, root, templates }");
  }
  if (typeof input.model !== "string") {
    throw new TypeError("model must be a string");
  }
  if (typeof input.root !== "string") {
    throw new TypeError("root must be a string");
  }
  if (!Array.isArray(input.templates) || !input.templates.every((t) => typeof t === "string")) {
    throw new TypeError("templates must be an array of strings");
  }
}

/** One call, on the current instance, discarding it if it fails. */
async function runNow<T>(
  inputs: readonly Uint8Array[],
  call: (exports: Exports, pointers: number[]) => number,
  shape: (value: unknown) => T,
): Promise<T> {
  const exports = await current();
  try {
    return shape(transfer(exports, inputs, (pointers) => call(exports, pointers)));
  } catch (cause) {
    // The instance may be half-updated: never call it again (outline D6).
    if (instance === exports) {
      instance = undefined;
    }
    if (cause instanceof MeshInternalError) {
      throw cause;
    }
    throw new MeshInternalError("the compiler failed", { cause });
  }
}

function enqueue<T>(work: () => Promise<T>): Promise<T> {
  const run = queue.then(work);
  queue = run.catch(() => undefined);
  return run;
}

/** Checks `input`. See the package's `check`. */
export function check(input: CheckInput): Promise<DiagnosticsDocument> {
  return enqueue(async () => {
    validate(input, false);
    return runNow(
      checkInputs(input),
      (exports, pointers) => exports.mesh_check(...pointers, input.model ? 1 : 0),
      checkResult,
    );
  });
}

/** Compiles `input`. See the package's `compile`. */
export function compile(input: CompileInput): Promise<CompileResult> {
  return enqueue(async () => {
    validate(input, true);
    return runNow(
      checkInputs(input),
      (exports, pointers) => exports.mesh_compile(...pointers),
      compileResult,
    );
  });
}

/** Checks a program. See the package's `checkProgram`. */
export function checkProgram(input: ProgramInput): Promise<RuntimeDiagnosticsDocument> {
  return enqueue(async () => {
    validateProgram(input);
    return runNow(
      [encoder.encode(input.model), encoder.encode(input.root), textList(input.templates)],
      (exports, pointers) => exports.mesh_check_program(...pointers),
      programResult,
    );
  });
}

/**
 * The instance in use, if any, for the package's own memory and failure
 * tests. Not API.
 */
export function instanceForTests(): Record<string, unknown> | undefined {
  return instance as unknown as Record<string, unknown> | undefined;
}
