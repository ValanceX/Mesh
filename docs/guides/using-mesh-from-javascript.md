# Using MESH from JavaScript

`@valancex/mesh-compiler` is the MESH compiler in WebAssembly, for Node and browsers. It checks MPRX exactly as `mesh check` does, against the same component manifest, and returns the same diagnostics, as data. This guide builds the loop it was made for: something generates MPRX (often a model), MESH checks it, and the result goes back to whatever wrote it until the source is right.

**You'll need:** Node 22 or 24, or a browser with WebAssembly. No Rust toolchain.

```console
$ npm install @valancex/mesh-compiler
```

## One check

`check` takes the source, a path, and optionally a model: a manifest's text, its path, and the component whose template the source is. It returns the diagnostics document:

```js
import { check } from "@valancex/mesh-compiler";

const document = await check({ source: "<page></pag>", path: "page.mprx" });
// { version: 1, diagnostics: [{ severity: "error", code: "mismatched-closing-tag", ... }] }
```

- **Every problem with your input is a diagnostic, never an exception.** Match on `code`; `message` is for people and may change.
- **Paths are names you choose.** They identify documents in diagnostics, and the package never reads them: there's no file system access, in Node or in a browser.
- **The component is always explicit.** MESH never guesses it from a path.
- **A diagnostic about the manifest** carries the manifest's `path`, and when the manifest has errors, nothing else is checked.

The document's shape is [`schemas/diagnostics-v1.schema.json`](../../schemas/diagnostics-v1.schema.json), the same one `mesh check --format json` prints, and the package ships TypeScript types for it.

## Positions, in JavaScript's units

Each span has a `start` and an `end`, and each position gives the same place in several units. JavaScript strings index UTF-16 code units, so use `utf16`:

```js
const { start, end } = diagnostic.span;
source.slice(start.utf16, end.utf16); // exactly the text the diagnostic points at
```

`line` and `utf16Column` (both 1-based) are what editors such as Monaco take. `byte`, `line` and `column` are what `mesh check` prints.

## Check, repair, check again

Diagnostics often carry suggestions: a replacement for a span, best first (`did you mean "user"?`). A generator can use them directly, or be shown the diagnostics and asked again. This example applies suggestions until the source checks clean:

```js
import { check } from "@valancex/mesh-compiler";

// The component model: which components exist, their props, and what a
// template can refer to. Usually a components.json you already have.
const manifest = JSON.stringify({
  version: 1,
  types: {
    User: {
      kind: "record",
      fields: {
        name: { type: { kind: "string" }, required: true },
        avatar: { type: { kind: "string" }, required: true },
      },
    },
  },
  components: {
    page: { props: { title: { type: { kind: "string" }, required: true } }, events: {}, commands: {}, scope: {} },
    text: { props: {}, events: {}, commands: {}, scope: {} },
    avatar: {
      props: {
        src: { type: { kind: "string" }, required: true },
        alt: { type: { kind: "string" }, required: true },
      },
      events: {},
      commands: {},
      scope: {},
    },
    profile: { props: {}, events: {}, commands: {}, scope: { user: { kind: "named", name: "User" } } },
  },
});

// What a generator wrote: the template of the `profile` component, with
// three mistakes in it.
let source = `<page title="Profile">
  <avatar src={user.avatar} atl={user.name} />
  <text>{usr.nmae}</text>
</page>`;

// Applies each error's best suggestion. Positions are in UTF-16 units, as
// JavaScript strings are, and applied from the end so earlier ones stay put.
function repair(source, document) {
  const edits = document.diagnostics
    .filter((diagnostic) => diagnostic.severity === "error" && diagnostic.suggestions.length > 0)
    .map((diagnostic) => diagnostic.suggestions[0])
    .sort((a, b) => b.span.start.utf16 - a.span.start.utf16);
  for (const { replacement, span } of edits) {
    source = source.slice(0, span.start.utf16) + replacement + source.slice(span.end.utf16);
  }
  return source;
}

for (let round = 1; ; round++) {
  const document = await check({
    source,
    path: "profile.mprx",
    model: { manifest, path: "components.json", component: "profile" },
  });
  const errors = document.diagnostics.filter((diagnostic) => diagnostic.severity === "error");
  console.log(`round ${round}: ${errors.map((error) => error.code).join(", ") || "no errors"}`);
  if (errors.length === 0) {
    break;
  }
  const repaired = repair(source, document);
  if (repaired === source || round === 5) {
    // Nothing left to apply: hand the diagnostics back to whatever wrote the source.
    throw new Error(errors.map((error) => error.message).join("\n"));
  }
  source = repaired;
}
console.log(source);
```

It prints:

```text
round 1: missing-required-prop, unknown-prop, unknown-reference
round 2: unknown-member
round 3: no errors
<page title="Profile">
  <avatar src={user.avatar} alt={user.name} />
  <text>{user.name}</text>
</page>
```

Two things to notice. Fixing one mistake can reveal the next: `usr.nmae` only reports the member once `usr` is `user`. And `missing-required-prop` has no suggestion of its own, but repairing `atl` fixes it. When no suggestion is left, the diagnostics themselves (codes, messages and positions) are what to hand back to the generator.

## In a browser

The package loads its WebAssembly module itself in Node. In a browser, load it once with `init`, from wherever your setup serves `@valancex/mesh-compiler/mesh.wasm`:

```js
import { check, init } from "@valancex/mesh-compiler";

await init(new URL("/assets/mesh.wasm", location.href));
const document = await check({ source, path: "page.mprx" });
```

`init` also takes the module's bytes or a compiled `WebAssembly.Module`. Bundler-specific loading isn't part of the package's contract.

## When something goes wrong

- A `TypeError`: an argument isn't a string, or a model has no `component`.
- `MeshVersionError`: the WebAssembly module isn't this version of the package's, for example a stale copy served in a browser. Nothing was checked.
- `MeshInternalError`: the compiler itself failed, which is a bug; please report it with the input. The package discards the failed compiler and uses a fresh one for the next check.

## What it doesn't do

It checks; it doesn't render or run anything, and it returns only diagnostics, not the compiled tree. For your editor, see the [editor setup guide](./editor-setup.md); for Rust, the [embedding guide](./embedding-the-compiler.md).
