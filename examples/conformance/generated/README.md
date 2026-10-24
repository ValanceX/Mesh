# `generated/`: checking a source while it arrives (decisions G1 to G14)

The data for streaming checks (`docs/manual/streaming.md`): programs a model might write, mistakes it might plant, and the digests of what a session reports at every prefix. Against [`components.json`](./components.json) (its own manifest, with command `effect` and `title` annotations), as the template of the component `page`.

| File | What it is |
|---|---|
| `pages/*.mprx` | six pages that check clean: a card with links, branches, a keyed list, nested blocks with accents and an emoji, a long text with strings holding `}` and `{`, and a repeat whose body is two elements |
| `mistakes.json` | `[{ name, code, source }]`: eight sources with one planted mistake each (an unknown prop, component, reference, command, event or variant, a value of the wrong type, a reference in a block header). Each must be heard within 20 bytes of where the mistake starts, once |
| `steps.expected.json` | for each page and each mistake, the number of prefixes (between characters) and the SHA-256 of the summary line of the step each gives: `cut`, what is open (`kind:name:start`), whether an `{#each}` was withheld, the diagnostics standing (`code@start-end`, sorted) and whether there is a preview |

The native test (`crates/mesh-compiler/tests/stream.rs`) records the digests (`MESH_BLESS=1`) and checks the laws; the JavaScript tests (`packages/mesh-compiler/test/stream.test.mjs`, `packages/mesh-runtime/test/generated.test.mjs`) compute the same digests through WebAssembly, render every preview with the runtime and compare it with the final tree. A PORT does not use this directory; a host that streams generated UI does, to check its own use of a session.
