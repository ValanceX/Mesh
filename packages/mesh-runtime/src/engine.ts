/**
 * The engine behind the public API: loading the WebAssembly module,
 * checking its version, keeping one instance, moving bytes in and out of
 * it, and discarding it after a failure.
 *
 * Deliberately boring (I6, I11): nothing here knows MPRX, the model,
 * values, trees or intents. It encodes (`encode.ts`), transfers, and
 * parses the one result document the module returns. Not public API: the
 * package's `exports` map doesn't expose this file, except for the types
 * and classes `index.ts` re-exports.
 */

import { encodeTexts, encodeValue } from "./encode.js";
import type { CommandIntent, RenderTree, RuntimeDiagnosticsDocument } from "./types.js";
import { version } from "./version.js";

/**
 * A failure at the boundary with the runtime: a trap (a Rust panic, or
 * running out of memory), or a result that isn't what the module
 * promises. It is never how the runtime reports a problem with its
 * input; those are diagnostics. The failed instance is discarded, and
 * the next call uses a fresh one.
 */
export class MeshInternalError extends Error {
  override name = "MeshInternalError";
}

/**
 * The WebAssembly module isn't the version this package was built for, or
 * isn't a MESH runtime module at all. Nothing runs against it.
 */
export class MeshVersionError extends Error {
  override name = "MeshVersionError";
}

/** What {@link init} accepts: where the module is, its bytes, or the module. */
export type ModuleSource = URL | string | BufferSource | WebAssembly.Module;

/** A program: the root component, and the templates (`template-v1` documents, as text). */
export interface ProgramInput {
  readonly root: string;
  readonly templates: readonly string[];
}

/** One render's inputs. */
export interface RenderInput {
  readonly program: ProgramInput;
  /** The manifest's text: the model the templates were compiled against. */
  readonly model: string;
  /** The root's scope values, by scope name: a plain object. */
  readonly snapshot: Record<string, unknown>;
}

/** What render gives: a render, or diagnostics. */
export type RenderResult =
  | { readonly render: Render; readonly diagnostics?: never }
  | { readonly render?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** What dispatch gives: a command intent, or diagnostics. */
export type DispatchResult =
  | { readonly intent: CommandIntent; readonly diagnostics?: never }
  | { readonly intent?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** Only the engine makes renders. */
const MAKING = Symbol("making a render");

/**
 * One successful render: its tree, and the program, model and snapshot
 * it came from, kept as they were given, with the snapshot encoded when
 * render was called, so later changes to the host's objects can't reach
 * it. Dispatch evaluates against exactly this snapshot.
 */
export class Render {
  /** The render tree, for a renderer. Frozen. */
  readonly tree: RenderTree;
  readonly #root: string;
  readonly #templates: Uint8Array;
  readonly #model: string;
  readonly #snapshot: Uint8Array;

  /** Not public: renders come from {@link render}. */
  constructor(
    token: symbol,
    tree: RenderTree,
    root: string,
    templates: Uint8Array,
    model: string,
    snapshot: Uint8Array,
  ) {
    if (token !== MAKING) {
      throw new TypeError("a Render comes from render()");
    }
    this.tree = tree;
    this.#root = root;
    this.#templates = templates;
    this.#model = model;
    this.#snapshot = snapshot;
    Object.freeze(this);
  }

  /** The inputs dispatch passes back to the module. Throws a TypeError for a foreign object. */
  static inputsOf(render: Render): [string, Uint8Array, string, Uint8Array] {
    if (!(typeof render === "object" && render !== null && #root in render)) {
      throw new TypeError("dispatch() takes a Render that render() returned");
    }
    return [render.#root, render.#templates, render.#model, render.#snapshot];
  }
}

/** The module's exports this engine uses. They're internal to the package. */
interface Exports {
  memory: WebAssembly.Memory;
  mesh_version_ptr(): number;
  mesh_version_len(): number;
  mesh_alloc(len: number): number;
  mesh_free(ptr: number, len: number): void;
  mesh_render(...args: number[]): number;
  mesh_dispatch(...args: number[]): number;
  mesh_result_ptr(): number;
  mesh_result_len(): number;
  mesh_result_clear(): void;
}

const EXPORTS = [
  "mesh_alloc",
  "mesh_free",
  "mesh_render",
  "mesh_dispatch",
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
/** Calls run one at a time, in call order. */
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
    throw new Error(`could not fetch the MESH runtime module from ${location.href}: ${response.status}`);
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

/** Instantiates `module` and checks it's a MESH runtime module of this version (I10). */
async function instantiate(module: WebAssembly.Module): Promise<Exports> {
  let exports: Record<string, unknown>;
  try {
    exports = (await WebAssembly.instantiate(module, {})).exports;
  } catch (cause) {
    throw new MeshVersionError("this WebAssembly module isn't a MESH runtime module", { cause });
  }
  const { memory, mesh_version_ptr, mesh_version_len } = exports;
  if (
    !(memory instanceof WebAssembly.Memory) ||
    typeof mesh_version_ptr !== "function" ||
    typeof mesh_version_len !== "function"
  ) {
    throw new MeshVersionError("this WebAssembly module isn't a MESH runtime module: it has no version");
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
      `this package is @valancex/mesh-runtime ${version}, but its WebAssembly module is ${found}`,
    );
  }
  for (const name of EXPORTS) {
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
 * package loads its own module on the first call, so this is only needed
 * to use another copy; in a browser, it's needed once, before the first
 * call. If it fails, nothing is kept.
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
        "@valancex/mesh-runtime: call init() with the URL of mesh-runtime.wasm before the first render",
      );
    }
    const module = await compileModule(new URL("./mesh-runtime.wasm", import.meta.url));
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

/** The module refused a host's value outright: reported as a `TypeError`. */
class Refused {
  constructor(readonly message: string) {}
}

/**
 * One call on `exports`: copies `inputs` in, calls `call` with each
 * one's address and length, frees them, and returns the result text, or
 * a {@link Refused}. Any exception means the instance failed.
 */
function transfer(
  exports: Exports,
  inputs: readonly Uint8Array[],
  call: (pointers: number[]) => number,
): string | Refused {
  const buffers = inputs.map((bytes) => ({ ptr: put(exports, bytes), len: bytes.length }));
  const status = call(buffers.flatMap(({ ptr, len }) => [ptr, len]));
  for (const { ptr, len } of buffers) {
    exports.mesh_free(ptr, len);
  }
  if (status !== 0 && status !== 2) {
    throw new MeshInternalError(`the runtime refused its input (status ${status})`);
  }
  const bytes = new Uint8Array(
    exports.memory.buffer,
    exports.mesh_result_ptr(),
    exports.mesh_result_len(),
  ).slice();
  exports.mesh_result_clear();
  const text = decoder.decode(bytes);
  return status === 2 ? new Refused(text) : text;
}

/** Parses the module's result document, freezing every object and array in it. */
function parse(text: string): Record<string, unknown> {
  const value: unknown = JSON.parse(text, (_key, item: unknown) =>
    typeof item === "object" && item !== null ? Object.freeze(item) : item,
  );
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new MeshInternalError("the runtime's result isn't a result document");
  }
  return value as Record<string, unknown>;
}

function isDiagnostics(value: unknown): value is RuntimeDiagnosticsDocument {
  return (
    typeof value === "object" &&
    value !== null &&
    Array.isArray((value as { diagnostics?: unknown }).diagnostics)
  );
}

/** One call on the current instance, discarding it if it fails. */
async function runNow<T>(
  inputs: readonly Uint8Array[],
  call: (exports: Exports, pointers: number[]) => number,
  shape: (result: Record<string, unknown>) => T,
): Promise<T> {
  const exports = await current();
  const failed = (cause: unknown): never => {
    // The instance may be half-updated: never call it again.
    if (instance === exports) {
      instance = undefined;
    }
    if (cause instanceof MeshInternalError) {
      throw cause;
    }
    throw new MeshInternalError("the runtime failed", { cause });
  };
  let result: string | Refused;
  try {
    result = transfer(exports, inputs, (pointers) => call(exports, pointers));
  } catch (cause) {
    return failed(cause);
  }
  if (result instanceof Refused) {
    // The runtime refused the host's value outright; the instance is fine.
    throw new TypeError(result.message);
  }
  try {
    return shape(parse(result));
  } catch (cause) {
    return failed(cause);
  }
}

function enqueue<T>(work: () => Promise<T>): Promise<T> {
  const run = queue.then(work);
  queue = run.catch(() => undefined);
  return run;
}

function validateRender(input: RenderInput): void {
  if (typeof input !== "object" || input === null) {
    throw new TypeError("render() takes an object: { program, model, snapshot }");
  }
  const program = input.program;
  if (typeof program !== "object" || program === null) {
    throw new TypeError("program must be an object: { root, templates }");
  }
  if (typeof program.root !== "string") {
    throw new TypeError("program.root must be a string");
  }
  if (!Array.isArray(program.templates) || !program.templates.every((t) => typeof t === "string")) {
    throw new TypeError("program.templates must be an array of strings");
  }
  if (typeof input.model !== "string") {
    throw new TypeError("model must be a string");
  }
}

/** Renders. See the package's `render`. */
export function render(input: RenderInput): Promise<RenderResult> {
  return enqueue(async () => {
    validateRender(input);
    const root = input.program.root;
    const templates = encodeTexts(input.program.templates);
    const model = input.model;
    const snapshot = encodeValue(input.snapshot);
    return runNow(
      [encoder.encode(root), templates, encoder.encode(model), snapshot],
      (exports, pointers) => exports.mesh_render(...pointers),
      (result): RenderResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (typeof result.tree !== "object" || result.tree === null) {
          throw new MeshInternalError("the runtime's result is neither a tree nor diagnostics");
        }
        const tree = result.tree as RenderTree;
        return Object.freeze({ render: new Render(MAKING, tree, root, templates, model, snapshot) });
      },
    );
  });
}

/** Dispatches. See the package's `dispatch`. */
export function dispatch(render: Render, handler: string, payload?: unknown): Promise<DispatchResult> {
  return enqueue(async () => {
    const [root, templates, model, snapshot] = Render.inputsOf(render);
    if (typeof handler !== "string") {
      throw new TypeError("handler must be a string: a handler identifier from the render's tree");
    }
    const absent = payload === undefined;
    const encoded = absent ? new Uint8Array(0) : encodeValue(payload);
    return runNow(
      [encoder.encode(root), templates, encoder.encode(model), snapshot, encoder.encode(handler), encoded],
      (exports, pointers) => exports.mesh_dispatch(...pointers, absent ? 0 : 1),
      (result): DispatchResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (typeof result.intent !== "object" || result.intent === null) {
          throw new MeshInternalError("the runtime's result is neither an intent nor diagnostics");
        }
        return Object.freeze({ intent: result.intent as CommandIntent });
      },
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
