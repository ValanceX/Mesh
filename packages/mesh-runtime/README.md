# @valancex/mesh-runtime

The MESH runtime, in WebAssembly. It renders a **program** of MESH templates against a **snapshot** of your values, producing a render tree for a renderer to draw, and turns the events the renderer reports into **command intents** for your application. It's the same Rust runtime as MESH's native one, so the same inputs give the same results everywhere.

```js
import { render, dispatch } from "@valancex/mesh-runtime";

const result = await render({
  program: { root: "users", templates },   // template-v1 documents, as text
  model: manifest,                          // the component manifest's text
  snapshot: { users, selected },           // the root's scope values
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

- **`render({ program, model, snapshot })`** returns a promise of `{ render }` or `{ diagnostics }`. `render.tree` is the render tree ([`schemas/render-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/render-v1.schema.json)), frozen: primitive nodes, final prop values, text and handler identifiers, and nothing to compute. `diagnostics` is the runtime diagnostics document ([`schemas/runtime-diagnostics-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/runtime-diagnostics-v1.schema.json)): every problem with the program or the snapshot, each at its location, or the first evaluation error.
- **`dispatch(render, handler, payload?)`** returns a promise of `{ intent }` or `{ diagnostics }`. The intent names the command and holds its evaluated arguments. It evaluates against the snapshot `render` was made from, never a newer one.
- **`init(source)`** loads the WebAssembly module from a URL, its bytes, or a `WebAssembly.Module`. In Node the package loads its own module; in a browser, call it once first with the URL of `@valancex/mesh-runtime/mesh-runtime.wasm`.
- **`version`** is the package's version, which is also its module's.

A problem with your values is a diagnostic, never an exception. The promises reject only with a `TypeError` for arguments of the wrong type (including a snapshot that isn't a plain object, or a `render` this package didn't make), with `MeshVersionError` for a module of another version, and with `MeshInternalError` if the runtime itself fails; after that, the next call uses a fresh instance.

## Your obligations as a host

- **Keep each render while its tree is drawn,** and dispatch an event with the render whose tree the renderer drew when it fired. A render keeps its own copy of its snapshot, taken when `render` was called, so changing your objects afterwards changes nothing.
- **Tell your renderer when a tree comes from a different program.** Keys are stable within one program, and change when the program does.
- **Give values as plain data:** `null`, booleans, finite numbers, strings, arrays and plain objects. A missing property and one that is `undefined` are both absent. Anything else (a `Map`, a `Date`, a class instance, a function, a `bigint`, NaN, a hole in an array, a string with an unpaired surrogate, a cycle) reaches the runtime as it is, and the runtime reports it.

## Support

Node 22 and later, and current browsers.

## License

MIT
