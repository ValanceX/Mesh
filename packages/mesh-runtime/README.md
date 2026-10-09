# @valancex/mesh-runtime

The MESH runtime, in WebAssembly. It renders a **program** of MESH templates against a **snapshot** of your values, producing a render tree for a renderer to draw, and turns the events the renderer reports into **command intents** for your application. It's the same Rust runtime as MESH's native one, so the same inputs give the same results everywhere.

```js
import { render, dispatch } from "@valancex/mesh-runtime";

const result = await render({
  program: { root: "users", templates },   // template-v1 documents, as text
  model: manifest,                          // the component manifest's text
  snapshot,                                 // the root's scope values, by name
});
if (result.diagnostics) {
  throw new Error(result.diagnostics.diagnostics[0].message);
}
draw(result.render.tree);

// When the renderer reports an event on that tree:
const { intent, diagnostics } = await dispatch(result.render, handler, payload);
```

Templates come from the compiler: `mesh compile`, or `compile()` in [`@valancex/mesh-compiler`](https://www.npmjs.com/package/@valancex/mesh-compiler). This package has no dependencies, and doesn't include the compiler.

## The API

- **`render({ program, model, snapshot })`** returns a promise of `{ render }` or `{ diagnostics }`. `render.tree` is the render tree ([`schemas/render-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/render-v1.schema.json)), frozen: primitive nodes, final prop values with the MESH text of each number, boolean and `null` prop (`propText`), text and handler identifiers, and nothing to compute: a renderer realizes a prop natively or with that text, and never converts a value itself. `diagnostics` is the runtime diagnostics document ([`schemas/runtime-diagnostics-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/runtime-diagnostics-v1.schema.json)): every problem with the program or the snapshot, each at its location, or the first evaluation error.
- **`dispatch(render, handler, payload?)`** returns a promise of `{ intent }` or `{ diagnostics }`. The intent names the command and holds its evaluated arguments. It evaluates against the snapshot `render` was made from, never a newer one.
- **`update(render, snapshot)`** returns a promise of `{ render, patches }` or `{ diagnostics }`: the render of a new snapshot of the same program, and the `render-patch-v1` patches ([`schemas/render-patch-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/render-patch-v1.schema.json)) that turn `render`'s tree into its tree. Applying them in order gives exactly the tree `render()` would give; a renderer applies lists and never compares them. `setProp`, `removeProp` and `setText` change a kept part, `insert`, `remove` and `move` add, take away and reorder parts by key, and `replace` (a tree nothing else can turn into the next) is the only operation of its list. The module keeps the render `update` returns (and `render` keeps nothing), so the next `update` from it doesn't derive it again; see **`render.release()`**. It returns the patches and not the new tree: the package applies them to the previous tree, so an update costs what changed, and the new tree shares every part it didn't reach with the previous one (a part that didn't change is the same object). On a keyed list of 10,000 items with one changed, an update is about 60 ms against about 470 ms to render (`node scripts/bench-update.mjs`). A refused update leaves `render` untouched and still valid.
- **`updateChanges(render, { base, changes }, { verify }?)`** is `update` for a host that knows what it changed: `changes` is a list of edits, `{ op: "set" | "insert" | "remove", path, value? }`, at paths into the snapshot (a scope name, then record fields and list indices), and `base` is the `render.version` of the render they are for. The snapshot isn't encoded or validated again, only the values the edits give are, and the result is `{ render, patches }` or `{ diagnostics }` as for `update`. The edits define the new snapshot and apply all or none; changes for another render are refused (`runtime-changes-base-mismatch`). With `verify` (the whole snapshot you believe you now have) a mismatch is refused too (`runtime-changes-disagree`): it costs a full validation, so use it in tests. `previous` may be any render. If the module holds it, the changes apply there. If not (`render()` made it, or it was released), the module derives it again under its version, which costs a render more. Only a render `updateChanges` made has no snapshot of its own: once it is released, or the module is replaced, `update`, `updateChanges` and `dispatch` reject with `MeshUsageError` (`render-gone`). On a keyed list of 10,000 items with one changed, about 22 ms against about 60 for `update` (`node scripts/bench-update.mjs`). A render made from changes has its snapshot only in the module, so it can be dispatched with and updated while the module holds it, and not after `release()`.
- **`diff(previous, next)`** returns the `changes` list that turns one snapshot into the other, for a host that has both and no record of what changed. Numbers compare by their bits (`0` and `-0` differ); an unchanged value gives no edit, a list changed at its ends gives edits at its ends, and a value that isn't a plain record, list or scalar is given whole for the runtime to judge. Nothing the runtime does calls it.
- **`render.version`** is the `RenderVersion` `updateChanges` takes as `base`; each render has its own. The type is a branded number: take it from `render.version`, don't write one.
- **`render.release()`** ends the module's copy of a render that `update` or `updateChanges` made (`render` keeps nothing, so releasing its result does nothing). Call it on a render you will no longer update from or dispatch with (typically the previous one, once its patches are applied). It is safe to call twice, a released render still works (its next `update` derives it again), except a render `updateChanges` made, which has no snapshot outside the module (`render-gone`), and a render you never release is released when it is garbage collected, which can be late.
- **`declaredEvents({ program, model })`** returns a promise of `{ events }` or `{ diagnostics }`. The events are the ones the program's templates declare, each with its declaring `component`, `event`, `command` and source `span`, whether or not a render contains them: those in composites, in inactive `mesh-if` alternatives and in `mesh-each` bodies are there, and nothing is deduplicated. It needs no snapshot, and gives the diagnostics `render` would for an invalid program. See [The declared events](https://github.com/ValanceX/Mesh/blob/main/docs/manual/runtime.md#the-declared-events).
- **`init(source)`** loads the WebAssembly module from a URL, its bytes, or a `WebAssembly.Module`. Optional: the package loads its own module (the file next to its code) on the first call, in Node and in a browser. Call `init` to load another copy or to give the URL yourself, for example of `@valancex/mesh-runtime/mesh-runtime.wasm` as a bundler resolves it; a Vite 5 to 7 development server may need `@valancex/mesh-runtime` excluded from dependency optimization (`optimizeDeps.exclude`) for the automatic path to find its file.
- **`version`** is the package's version, which is also its module's.

A problem with your values is a diagnostic, never an exception. A call to `render`, `dispatch`, `update`, `updateChanges` or `declaredEvents` rejects only with:

| Error | When | `code` |
|---|---|---|
| `MeshUsageError` (a `TypeError`, so existing `catch` on `TypeError` still works) | an argument of the wrong type (including a snapshot that isn't a plain object) | `invalid-argument` |
| | a `Render` this package didn't make | `not-a-render` |
| | a render whose copy in the module is gone and which has no snapshot of its own (see `updateChanges`) | `render-gone` |
| `MeshVersionError` | a module of another version, or not a MESH module | `version-mismatch` |
| `MeshInternalError` | the runtime itself failed; the next call uses a fresh instance | `internal-error` |

`init(source)` rejects the same way: `MeshUsageError` `invalid-argument` for a source of the wrong type, `module-unavailable` when the module can't be read or fetched (also the code of the browser's "could not load its packaged module" error), and `MeshVersionError` for bytes that aren't a WebAssembly module or a module of another version.

Match on `code`, never on `message`. Codes are stable: never renamed, never reused for another meaning.

**Calls and concurrency.** Calls to the package run one at a time, in the order they were made, whether or not you `await` each: a second call starts when the first has settled, and a call that rejects doesn't stop the ones after it. There is no cancellation: a call that has been made runs to completion. Nothing the package does continues after its promise settles, except the release of a render's copy in the module, which `release()` queues behind the calls already made.

**Cleanup.** `render.release()` is synchronous from the caller's side and safe to call twice. Where the platform has `Symbol.dispose`, a render has `[Symbol.dispose]()`, which does the same, so `using render = ...` releases it at the end of a block. The `using` statement itself needs a runtime or transpiler with explicit resource management (it is a syntax error on Node 22); otherwise call `release()` in a `finally`.

## Your obligations as a host

- **Keep each render while its tree is drawn,** and dispatch an event with the render whose tree the renderer drew when it fired. Release a render when you are done with it (see `render.release()`): the module keeps each render `update` or `updateChanges` returns until you do, or until it is garbage collected, which can be late; a loop that updates thousands of times should release as it goes. A render keeps its own copy of its snapshot, taken when `render` was called, so changing your objects afterwards changes nothing.
- **Tell your renderer when a tree comes from a different program.** A key names the same node in every render of one program that has that node, and keys change when the program does.
- **Give values as plain data:** `null`, booleans, finite numbers, strings, arrays and plain objects. A missing property and one that is `undefined` are both absent. Anything else (a `Map`, a `Date`, a class instance, a function, a `bigint`, NaN, a hole in an array, a string with an unpaired surrogate, a cycle) reaches the runtime as it is, and the runtime reports it.

## Guides

- [Integrating MESH with NEXUS](https://github.com/ValanceX/Mesh/blob/main/docs/guides/integrating-mesh-with-nexus.md): a host, end to end.
- [Rendering MESH output](https://github.com/ValanceX/Mesh/blob/main/docs/guides/rendering-mesh-output.md): a renderer, end to end.
- [The runtime manual](https://github.com/ValanceX/Mesh/blob/main/docs/manual/runtime.md): the whole contract.

## Support

Node 22 and later, and current browsers.

## License

MIT
