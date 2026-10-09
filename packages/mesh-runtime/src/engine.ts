/**
 * The engine behind the public API: loading the WebAssembly module,
 * checking its version, keeping one instance, moving bytes in and out of
 * it, and discarding it after a failure.
 *
 * Deliberately boring (I6, I11): nothing here knows MPRX, the model,
 * values, trees or intents. It encodes (`encode.ts`), transfers, and
 * parses the one result document the module returns, and, for `update`,
 * hands the patches in it to `patches.ts`. Not public API: the
 * package's `exports` map doesn't expose this file, except for the types
 * and classes `index.ts` re-exports.
 */

import { encodeTexts, encodeValue } from "./encode.js";
import { applyPatches } from "./patches.js";
import type {
  CommandIntent,
  DeclaredEvent,
  RenderPatches,
  RenderTree,
  RuntimeDiagnosticsDocument,
} from "./types.js";
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
  /**
   * Keep the render in the module, as `update` does for the render it makes, so
   * that {@link updateChanges} can apply changes to it. The render then has a
   * `version`, which those changes name, and the host releases it
   * (`render.release()`) when it is done with it.
   */
  readonly keep?: boolean;
}

/** One step of a change's path: a record field's name or a list index. */
export type PathStep = string | number;

/** One edit of a snapshot, at a path into it. See `updateChanges`. */
export type Change =
  | { readonly op: "set"; readonly path: readonly PathStep[]; readonly value: unknown }
  | { readonly op: "insert"; readonly path: readonly PathStep[]; readonly value: unknown }
  | { readonly op: "remove"; readonly path: readonly PathStep[] };

/** The edits to a render's snapshot, and the render they were computed against. */
export interface Changes {
  /** The `version` of the render these changes are for. Changes for any other render are refused. */
  readonly base: number;
  /** The edits, applied in order: an edit sees the effect of those before it. */
  readonly changes: readonly Change[];
}

/** One declared-events request: a program, and the manifest it was compiled against. */
export interface DeclaredEventsInput {
  readonly program: ProgramInput;
  /** The manifest's text: the model the templates were compiled against. */
  readonly model: string;
}

/** What declaredEvents gives: the program's declared events, or diagnostics. */
export type DeclaredEventsResult =
  | { readonly events: readonly DeclaredEvent[]; readonly diagnostics?: never }
  | { readonly events?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** What render gives: a render, or diagnostics. */
export type RenderResult =
  | { readonly render: Render; readonly diagnostics?: never }
  | { readonly render?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** What update gives: the new render and the patches from the previous tree to its tree, or diagnostics. */
export type UpdateResult =
  | { readonly render: Render; readonly patches: RenderPatches; readonly diagnostics?: never }
  | { readonly render?: never; readonly patches?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** What dispatch gives: a command intent, or diagnostics. */
export type DispatchResult =
  | { readonly intent: CommandIntent; readonly diagnostics?: never }
  | { readonly intent?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/** The module's copy of a render: its handle, the module it is in, and the render's version. */
interface Kept {
  readonly handle: number;
  readonly generation: number;
  readonly version: number;
}

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
  /** The snapshot as encoded, if this render has one; a render made from changes has none outside the module. */
  readonly #snapshot: Uint8Array | undefined;
  /** The module's copy of this render: its handle, the module it is in, and its version. */
  #kept: Kept | undefined;

  /** Not public: renders come from {@link render} and {@link update}. */
  constructor(
    token: symbol,
    tree: RenderTree,
    root: string,
    templates: Uint8Array,
    model: string,
    snapshot: Uint8Array | undefined,
    kept?: Kept,
  ) {
    if (token !== MAKING) {
      throw new TypeError("a Render comes from render(), update() or updateChanges()");
    }
    this.tree = tree;
    this.#root = root;
    this.#templates = templates;
    this.#model = model;
    this.#snapshot = snapshot;
    this.#kept = kept;
    if (kept) {
      // If this render is dropped without release(), its copy in the module is released when it is collected.
      keptRegistry.register(this, kept, this);
    }
    Object.freeze(this);
  }

  /**
   * The number changes name to say which render they were computed against,
   * while the module holds this render (it was made by `update` or
   * `updateChanges`, or by `render` with `keep`, and not released); otherwise
   * `undefined`.
   */
  get version(): number | undefined {
    return this.#kept?.version;
  }

  /**
   * Releases the module's copy of this render, which `update` made so the
   * next `update` need not derive it again. Call it when this render is no
   * longer the one to update from or to dispatch with: once an update's
   * patches are applied, release the render before it. It is safe to call
   * twice, and a render still works after it, since it keeps its own inputs:
   * the next `update` just derives it again, as it does for a render
   * `render()` made. **Except a render made by `updateChanges`:** it has no
   * snapshot outside the module, so once released (or if the module is
   * replaced) it can no longer be updated or dispatched with; render again from
   * a whole snapshot. A render that is never released is released when it is
   * garbage collected, but that is up to the engine and can be late.
   */
  release(): void {
    const kept = this.#kept;
    if (!kept) {
      return;
    }
    this.#kept = undefined;
    keptRegistry.unregister(this);
    releaseKept(kept);
  }

  /** The module's copy of `render`, if it has one that is still in the module in use. Throws a TypeError for a foreign object. */
  static keptOf(render: Render): Kept | undefined {
    if (!(typeof render === "object" && render !== null && #root in render)) {
      throw new TypeError("update() takes a Render that render(), update() or updateChanges() returned");
    }
    return render.#kept;
  }

  /** The inputs dispatch passes back to the module. Throws a TypeError for a foreign object. */
  static inputsOf(render: Render): [string, Uint8Array, string, Uint8Array | undefined] {
    if (!(typeof render === "object" && render !== null && #root in render)) {
      throw new TypeError("dispatch() takes a Render that render(), update() or updateChanges() returned");
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
  mesh_update(...args: number[]): number;
  mesh_render_kept(...args: number[]): number;
  mesh_update_changes(...args: number[]): number;
  mesh_dispatch_kept(...args: number[]): number;
  mesh_release(handle: number): void;
  mesh_dispatch(...args: number[]): number;
  mesh_declared_events(...args: number[]): number;
  mesh_result_ptr(): number;
  mesh_result_len(): number;
  mesh_result_clear(): void;
}

const EXPORTS = [
  "mesh_alloc",
  "mesh_free",
  "mesh_render",
  "mesh_update",
  "mesh_render_kept",
  "mesh_update_changes",
  "mesh_dispatch_kept",
  "mesh_release",
  "mesh_dispatch",
  "mesh_declared_events",
  "mesh_result_ptr",
  "mesh_result_len",
  "mesh_result_clear",
] as const;

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/**
 * Which instance each `Exports` is, in order of creation. A handle is only
 * good in the instance that made it, and an instance is discarded after a
 * boundary failure, so a render's handle names its instance's number.
 */
const generations = new WeakMap<Exports, number>();
let generationCount = 0;

/** Releases the module's copy of a render, in the instance that has it, if that is still the one in use. Never throws. */
function releaseKept(kept: Kept): void {
  void enqueue(async () => {
    if (instance && generations.get(instance) === kept.generation) {
      instance.mesh_release(kept.handle);
    }
  }).catch(() => undefined);
}

/** Releases the copy of a render that was garbage collected without `release()`. */
const keptRegistry = new FinalizationRegistry<Kept>(releaseKept);

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
  const made = { ...exports } as unknown as Exports;
  generations.set(made, ++generationCount);
  return made;
}

/**
 * Loads the module from `source` and checks its version. Without it, the
 * package loads its own module (the file next to this one) on the first
 * call, in Node and in a browser: `init` is for another copy, or another
 * location. If it fails, nothing is kept.
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
    let module: WebAssembly.Module;
    try {
      module = await compileModule(new URL("./mesh-runtime.wasm", import.meta.url));
    } catch (cause) {
      if (isNode()) {
        throw cause;
      }
      const detail = cause instanceof Error ? ` (${cause.message})` : "";
      throw new Error(
        `@valancex/mesh-runtime: could not load its packaged WebAssembly module automatically${detail}. ` +
          "Pass the URL of mesh-runtime.wasm to init() to load it explicitly. " +
          "If this is a Vite 5-7 development server, it may have moved this package into its dependency cache, " +
          "away from mesh-runtime.wasm: excluding @valancex/mesh-runtime from dependency optimization " +
          "(optimizeDeps.exclude) lets the package find its file.",
        { cause },
      );
    }
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
    const keep = input.keep === true;
    return runNow(
      [encoder.encode(root), templates, encoder.encode(model), snapshot],
      (exports, pointers) => (keep ? exports.mesh_render_kept(...pointers) : exports.mesh_render(...pointers)),
      (result): RenderResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (typeof result.tree !== "object" || result.tree === null) {
          throw new MeshInternalError("the runtime's result is neither a tree nor diagnostics");
        }
        const tree = result.tree as RenderTree;
        const kept = keep ? keptFrom(result) : undefined;
        return Object.freeze({ render: new Render(MAKING, tree, root, templates, model, snapshot, kept) });
      },
    );
  });
}

/** Whether the module in use still holds the render `kept` names. */
function liveIn(kept: Kept | undefined): kept is Kept {
  return kept !== undefined && instance !== undefined && generations.get(instance) === kept.generation;
}

/** What a render with no snapshot of its own is refused with when its copy in the module is gone. */
function gone(operation: string): TypeError {
  return new TypeError(
    `${operation}() needs this render's copy in the module, which is gone (the render was released, or the module was replaced), ` +
      "and a render made from changes has no snapshot of its own: render again from a whole snapshot",
  );
}

/** The module's copy of the render a result names. */
function keptFrom(result: Record<string, unknown>): Kept {
  const generation = instance === undefined ? undefined : generations.get(instance);
  if (typeof result.handle !== "number" || typeof result.version !== "number" || generation === undefined) {
    throw new MeshInternalError("the runtime's result names no render");
  }
  return { handle: result.handle, version: result.version, generation };
}

/** Updates. See the package's `update`. */
export function update(previous: Render, snapshotInput: Record<string, unknown>): Promise<UpdateResult> {
  return enqueue(async () => {
    const [root, templates, model, previousSnapshot] = Render.inputsOf(previous);
    const kept = Render.keptOf(previous);
    if (previousSnapshot === undefined && !liveIn(kept)) {
      throw gone("update");
    }
    const snapshot = encodeValue(snapshotInput);
    return runNow(
      [encoder.encode(root), templates, encoder.encode(model), previousSnapshot === undefined ? new Uint8Array(0) : previousSnapshot, snapshot],
      (exports, pointers) => {
        // The module's copy is good only in the instance that made it.
        const held = kept !== undefined && generations.get(exports) === kept.generation;
        return exports.mesh_update(...pointers, held ? kept.handle : 0, held ? 1 : 0);
      },
      (result): UpdateResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (typeof result.patches !== "object" || result.patches === null) {
          throw new MeshInternalError("the runtime's result is neither an update nor diagnostics");
        }
        // The module returns what changed; the new tree is the previous one with it applied.
        const patches = result.patches as RenderPatches;
        return Object.freeze({
          render: new Render(MAKING, applyPatches(previous.tree, patches), root, templates, model, snapshot, keptFrom(result)),
          patches,
        });
      },
    );
  });
}

/** Updates by changes. See the package's `updateChanges`. */
export function updateChanges(
  previous: Render,
  changes: Changes,
  options?: { readonly verify?: Record<string, unknown> },
): Promise<UpdateResult> {
  return enqueue(async () => {
    const [root, templates, model] = Render.inputsOf(previous);
    const kept = Render.keptOf(previous);
    // Changes are only ever applied to the render they name, which only the module that holds it can say.
    if (!liveIn(kept)) {
      throw new TypeError(
        "updateChanges() needs this render to be in the module: make it with update(), or render() with keep, and don't release it first",
      );
    }
    if (typeof changes !== "object" || changes === null) {
      throw new TypeError("updateChanges() takes the changes: { base, changes }");
    }
    const encoded = encodeValue(changes as unknown as Record<string, unknown>);
    const verify = options?.verify === undefined ? undefined : encodeValue(options.verify);
    return runNow(
      [encoder.encode(model), encoded, verify === undefined ? new Uint8Array(0) : verify],
      (exports, pointers) => exports.mesh_update_changes(...pointers, verify === undefined ? 0 : 1, kept.handle),
      (result): UpdateResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (typeof result.patches !== "object" || result.patches === null) {
          throw new MeshInternalError("the runtime's result is neither an update nor diagnostics");
        }
        const patches = result.patches as RenderPatches;
        // No snapshot is kept outside the module: the new render is the module's.
        return Object.freeze({
          render: new Render(MAKING, applyPatches(previous.tree, patches), root, templates, model, undefined, keptFrom(result)),
          patches,
        });
      },
    );
  });
}

function validateDeclaredEvents(input: DeclaredEventsInput): void {
  if (typeof input !== "object" || input === null) {
    throw new TypeError("declaredEvents() takes an object: { program, model }");
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

/** The events a program declares. See the package's `declaredEvents`. */
export function declaredEvents(input: DeclaredEventsInput): Promise<DeclaredEventsResult> {
  return enqueue(async () => {
    validateDeclaredEvents(input);
    return runNow(
      [encoder.encode(input.program.root), encodeTexts(input.program.templates), encoder.encode(input.model)],
      (exports, pointers) => exports.mesh_declared_events(...pointers),
      (result): DeclaredEventsResult => {
        if (isDiagnostics(result.diagnostics)) {
          return Object.freeze({ diagnostics: result.diagnostics });
        }
        if (!Array.isArray(result.events)) {
          throw new MeshInternalError("the runtime's result is neither declared events nor diagnostics");
        }
        return Object.freeze({ events: result.events as DeclaredEvent[] });
      },
    );
  });
}

/** Dispatches. See the package's `dispatch`. */
export function dispatch(render: Render, handler: string, payload?: unknown): Promise<DispatchResult> {
  return enqueue(async () => {
    const [root, templates, model, snapshot] = Render.inputsOf(render);
    const kept = Render.keptOf(render);
    if (typeof handler !== "string") {
      throw new TypeError("handler must be a string: a handler identifier from the render's tree");
    }
    const absent = payload === undefined;
    const encoded = absent ? new Uint8Array(0) : encodeValue(payload);
    // A render the module holds is dispatched there, against the snapshot it has; any other is dispatched from its own.
    const inModule = liveIn(kept);
    if (!inModule && snapshot === undefined) {
      throw gone("dispatch");
    }
    return runNow(
      inModule
        ? [encoder.encode(model), encoder.encode(handler), encoded]
        : [encoder.encode(root), templates, encoder.encode(model), snapshot as Uint8Array, encoder.encode(handler), encoded],
      (exports, pointers) =>
        inModule
          ? exports.mesh_dispatch_kept(...pointers, absent ? 0 : 1, kept.handle)
          : exports.mesh_dispatch(...pointers, absent ? 0 : 1),
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
