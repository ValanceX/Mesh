# @valancex/mesh-compiler

The [MESH](https://github.com/ValanceX/Mesh) compiler, built for WebAssembly. Check MPRX, with or without a component manifest, and get back exactly the diagnostics `mesh check --format json` prints for the same inputs.

Or compile it, and also get the component's **template**, exactly what `mesh compile` writes: the checked MPRX with every name resolved and no values, which the MESH runtime renders.

It's the Rust compiler itself, compiled to WebAssembly, not a reimplementation: this package adds no rule of its own.

```js
import { check } from "@valancex/mesh-compiler";

const document = await check({
  source: generated,          // the MPRX text
  path: "card.mprx",          // an identifier you choose; never read
  model: {                    // optional: check what the file means, too
    manifest: manifestText,   // the manifest's JSON text
    path: "components.json",  // an identifier you choose; never read
    component: "user-card",   // whose template `source` is: always explicit
  },
});

for (const diagnostic of document.diagnostics) {
  console.log(diagnostic.code, diagnostic.message);
  // source.slice(diagnostic.span.start.utf16, diagnostic.span.end.utf16) is the text it points at.
}
```

The [Using MESH from JavaScript](https://github.com/ValanceX/Mesh/blob/main/docs/guides/using-mesh-from-javascript.md) guide builds a generate-check-repair loop with it.

## The API

- **`check(input)`** returns a promise of the diagnostics document, the one [`schemas/diagnostics-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/diagnostics-v1.schema.json) describes. Anything wrong with your MPRX or your manifest is a diagnostic in it, never an exception. Diagnostics about the manifest carry the manifest's `path`. Each position has a `byte` offset, a 1-based `line` and `column`, and, for JavaScript, `utf16` (an offset into your string) and `utf16Column`.
- **`compile(input)`** takes `check`'s input, with `model` required, and returns a promise of `{ diagnostics, template? }`. `diagnostics` is the document `check` returns for the same input. `template` is the `template-v1` document ([`schemas/template-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/template-v1.schema.json)), present exactly when the diagnostics have no error; warnings don't stop it. A template is data to store and pass to the runtime, not to interpret. The `Template` type describes it.
- **`checkProgram({ model, root, templates })`** checks a program of templates: the manifest's text, the root component, and the templates' texts in order. It returns a promise of the runtime diagnostics document ([`schemas/runtime-diagnostics-v1.schema.json`](https://github.com/ValanceX/Mesh/blob/main/schemas/runtime-diagnostics-v1.schema.json)), empty when the program is valid: exactly what `mesh check-program --format json` prints, and what the runtime's render reports for the same program. The `RuntimeDiagnosticsDocument` type describes it.
- **`init(module)`** loads the WebAssembly module: a URL, its bytes, or a compiled `WebAssembly.Module`. In Node you don't need it; the package loads its own. In a browser, call it once before the first check, with the URL of `@valancex/mesh-compiler/mesh.wasm` as your setup serves it. Automatic loading by bundlers isn't part of this package's contract.
- **`version`**: the package's version.
- **`MeshVersionError`**: the WebAssembly module isn't this version's. No check runs against it.
- **`MeshInternalError`**: the compiler itself failed (a bug). The failed compiler instance is discarded, and the next check uses a fresh one.
- A `TypeError` for arguments of the wrong type.

Paths are identifiers: the package never touches the file system. Strings are passed as UTF-8, so a lone surrogate in a JavaScript string becomes U+FFFD, as with any UTF-8 encoding.

## Support

Node 22 and 24, and browsers with WebAssembly. ESM only, with TypeScript declarations.

## License

MIT
