# @valancex/mesh-conformance

MESH's conformance vectors as a data-only package. It has no code. A renderer (a PORT) checks itself against these files, and **never copies them**: a copy goes stale without anyone deciding to let it (the Web PORT's copy was at v0.6 and lacked the identity vectors).

The package is versioned with MESH. Its contents are exactly [`examples/conformance/`](../../examples/conformance/README.md), at `conformance/` in the tarball:

| Path | What |
|---|---|
| `values/`, `events/` | Real programs (`program.json`, `.mprx` sources, `snapshot.json`), their committed trees (`expected.tree.json`) and named cases (`cases.json`): how values become text, and which handler receives an interaction |
| `identity/cases.json` | Node identity across two renders (C1 to C10): kept, created, removed, moved |
| `identity/repeat/cases.json` | The same for repeated sites (C11 to C22) |
| `components.json` | The manifest the programs are checked against |
| `README.md` files | The format of each |

Each file is reachable through the package's exports:

```js
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const cases = JSON.parse(readFileSync(createRequire(import.meta.url).resolve("@valancex/mesh-conformance/identity/cases.json"), "utf8"));
```

`components.json` has a `$schema` that points into MESH's repository (`../../schemas/manifest-v1.schema.json`); the schema is not part of this package.

## How it is made

`npm pack` runs `scripts/prepack.mjs`, which copies `examples/conformance/` into `conformance/` (git-ignored). `test/pack.test.mjs` packs the package and checks that the tarball holds exactly those files, byte for byte, plus `package.json`, `README.md` and `LICENSE`, and nothing else.

It is not published yet. Until it is, a consumer installs the tarball (`npm pack` in this directory).
