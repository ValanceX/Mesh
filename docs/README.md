# MESH documentation

Documentation for **MESH v0.3**. New here? Start with *Getting started*.

## Guides

Learning-oriented walkthroughs.

- [**Getting started**](./guides/getting-started.md): install `mesh`, write and check your first file, and read diagnostics.
- [**Writing MPRX**](./guides/writing-mprx.md): a tour of everything the language supports, plus common mistakes and their fixes.
- [**Checking against a component model**](./guides/checking-against-a-component-model.md): declare your components in a manifest and check what a file means, not just its shape; requiredness, absence and `any`, explained.
- [**Editor setup**](./guides/editor-setup.md): diagnostics, hover, go to definition, completion and syntax highlighting in your editor, with a configuration verified in Neovim.
- [**Embedding the compiler**](./guides/embedding-the-compiler.md): call MESH from Rust, handle diagnostics, and walk the Semantic IR.

## Manuals

Reference material for when you need exact details.

- [**`mesh` CLI manual**](./manual/mesh-cli.md): commands, output streams, the human and JSON diagnostic formats, and exit codes.
- [**`mesh-lsp` manual**](./manual/mesh-lsp.md): the language server: installing, configuring it for your editor, and what it publishes when.
- [**Diagnostics reference**](./manual/diagnostics.md): every diagnostic code and message, what causes it, and how to fix it.
- [**Component manifest reference**](./manual/manifest.md): the manifest's format, version 1, and how it's validated. Also published as a [JSON Schema](../schemas/manifest-v1.schema.json).
- [**Diagnostics JSON Schema**](../schemas/diagnostics-v1.schema.json): the document `mesh check --format json` prints.
- [**MPRX Language Spec**](./MPRX-SPEC.md): the canonical grammar and semantics.

## Background

- [**Architecture**](./ARCHITECTURE.md): why MPRX looks the way it does, and how MESH fits into Valance.
- Release notes for [**v0.3**](./releases/v0.3.md), [**v0.2**](./releases/v0.2.md) and [**v0.1**](./releases/v0.1.md), and the [**CHANGELOG**](../CHANGELOG.md).

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

If you change `grammar/tree-sitter-mprx/grammar.js`, regenerate the parser with `npx tree-sitter generate` in that directory and commit `src/`; CI checks that it's up to date.

Expected CLI output lives next to each file in `examples/fixtures/`. If you change a diagnostic, update the matching `.stderr` file.
