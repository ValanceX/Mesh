# MESH documentation

Documentation for **MESH v0.5**. New here? Start with *Getting started*.

## Guides

Learning-oriented walkthroughs.

- [**Getting started**](./guides/getting-started.md): install `mesh`, write and check your first file, and read diagnostics.
- [**Writing MPRX**](./guides/writing-mprx.md): a tour of everything the language supports, plus common mistakes and their fixes.
- [**Checking against a component model**](./guides/checking-against-a-component-model.md): declare your components in a manifest and check what a file means, not just its shape; requiredness, absence and `any`, explained.
- [**Editor setup**](./guides/editor-setup.md): diagnostics, hover, go to definition, completion and syntax highlighting in your editor, with a configuration verified in Neovim.
- [**Using MESH from JavaScript**](./guides/using-mesh-from-javascript.md): check MPRX in Node or a browser with `@valancex/mesh-compiler`, and a generate-check-repair loop that applies MESH's suggestions.
- [**Embedding the compiler**](./guides/embedding-the-compiler.md): call MESH from Rust, handle diagnostics, and walk the Semantic IR.
- [**Integrating MESH with NEXUS**](./guides/integrating-mesh-with-nexus.md): build a host: compile templates, render from your state, pair each event with its render, and map intents to commands.
- [**Rendering MESH output**](./guides/rendering-mesh-output.md): build a renderer: draw a render tree as given, report events, and update by key.

## Manuals

Reference material for when you need exact details.

- [**`mesh` CLI manual**](./manual/mesh-cli.md): commands, output streams, the human and JSON diagnostic formats, and exit codes.
- [**`mesh-lsp` manual**](./manual/mesh-lsp.md): the language server: installing, configuring it for your editor, and what it publishes when.
- [**Diagnostics reference**](./manual/diagnostics.md): every diagnostic code and message, what causes it, and how to fix it.
- [**Component manifest reference**](./manual/manifest.md): the manifest's format, version 1, and how it's validated. Also published as a [JSON Schema](../schemas/manifest-v1.schema.json).
- [**Diagnostics JSON Schema**](../schemas/diagnostics-v1.schema.json): the document `mesh check --format json` prints.
- [**MPRX Language Spec**](./MPRX-SPEC.md): the canonical grammar and semantics.

**Templates and the runtime.** These are the contracts NEXUS's adapter and PORT's renderers build against. The compiler, the Rust runtime (`mesh-runtime`) and `@valancex/mesh-runtime` implement them:

- [**Evaluation and the boundary**](./MPRX-SPEC.md#97-evaluation): §9.7 and §9.8 of the spec, with the normative [number-to-text table](./tables/number-to-text.tsv) and its [reference generator](./tables/number_to_text.py).
- [**Templates and programs**](./manual/templates.md): the compiled form of a template, the model fingerprint, and the assembly rules. Also a [JSON Schema](../schemas/template-v1.schema.json). `mesh compile` and `compile()` emit templates; `mesh check-program` and `checkProgram()` check programs.
- [**The MESH runtime**](./manual/runtime.md): render and dispatch, the host's obligations, the render tree, keys, handler identifiers, command intents, and runtime diagnostics. Also JSON Schemas for [render trees and intents](../schemas/render-v1.schema.json) and [runtime diagnostics](../schemas/runtime-diagnostics-v1.schema.json).

## Background

- [**Architecture**](./ARCHITECTURE.md): why MPRX looks the way it does, and how MESH fits into Valance.
- Release notes for [**v0.5**](./releases/v0.5.md), [**v0.4**](./releases/v0.4.md), [**v0.3**](./releases/v0.3.md), [**v0.2**](./releases/v0.2.md) and [**v0.1**](./releases/v0.1.md), and the [**CHANGELOG**](../CHANGELOG.md).

## For contributors

`docs/superpowers/` holds the design specs (`specs/`) and step-by-step implementation plans (`plans/`) that MESH was built from. They record *how* each feature was decided and aren't needed for using MESH.

Before opening a pull request, run the same checks as CI:

```console
$ cargo fmt --all -- --check
$ cargo clippy --workspace --all-targets -- -D warnings
$ cargo test --workspace
$ RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

CI runs `cargo test --workspace` on Linux, macOS and Windows, so keep tests free of platform assumptions: build paths with `Path`, and don't expect `/` in a printed path. Every text file checks out with LF line endings on every platform (`.gitattributes`).

**Releases** are built by `.github/workflows/release.yml` from a `v*` tag: bump every version together (`crates/mesh-cli/tests/packages.rs` checks they agree), then push the tag. Publishing needs the `@valancex` npm organisation to exist (a token alone isn't enough: v0.4.0's first publish failed with `E404` until it did), and the `NPM_ACCESS_TOKEN` repository secret: an npm granular access token with read and write access to the `@valancex` scope. The token expires 60 days after it's created (the current one around 2026-11-24), and the release workflow's dry run can't tell: before tagging a release, renew it if it's close to expiry, and update the secret. A manual run of the workflow, without `publish`, is a dry run.

The npm packages under `packages/` are one npm workspace. Regenerate `packages/package-lock.json` with npm 11 or newer (`npx npm@11 install`), which records the unpublished platform packages npm 11's `npm ci` requires. If you change them, run, in `packages/`:

```console
$ npm ci
$ npm run build:wasm -w @valancex/mesh-compiler
$ npm run build:wasm -w @valancex/mesh-runtime
$ npm run build
$ npm run typecheck
$ npm test -w @valancex/mesh-compiler
$ npm test -w @valancex/mesh-runtime
```

`build:wasm` builds the compiler, or the runtime, for WebAssembly with Cargo; add the target once with `rustup target add wasm32-unknown-unknown`. Tree-sitter's C is compiled by `clang`, which needs its WebAssembly backend: LLVM's `clang` has it, and Apple's `clang` on macOS may not, so install LLVM there if the build can't find a `wasm32` target. The tests compare the package with a native `mesh` (`cargo build -p mesh-cli`, or `MESH_BIN`), the runtime's tests compare it with the native runtime's harness (`cargo build -p mesh-runtime-wasm --example harness`, or `MESH_RUNTIME_HARNESS`), and the browser test needs Playwright's Chromium (`npx playwright install chromium`, or `MESH_CHROMIUM` pointing at a Chromium).

If you change `grammar/tree-sitter-mprx/grammar.js`, regenerate the parser with `npx tree-sitter generate` in that directory and commit `src/`; CI checks that it's up to date.

Expected CLI output lives next to each file in `examples/fixtures/`. If you change a diagnostic, update the matching `.stderr` file.
