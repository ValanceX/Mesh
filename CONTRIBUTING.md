# Contributing to MESH

Bug reports and pull requests are welcome on [GitHub](https://github.com/ValanceX/Mesh/issues). This is how to work on MESH itself; to use it, start at the [documentation](docs/README.md).

## Documentation

Documentation lives in `docs/`, organized by what the reader wants to do: `tutorials/` (learn by building), `how-to/` (one task each), `reference/` (exact behavior) and `explanation/` (why, and the design record). A pull request that changes behavior updates the page that describes it, in the same pull request. Every page starts with a one-line summary and ends with where to go next.

## Checks and releases

`docs/explanation/specs/` and `docs/explanation/plans/` hold the design specs and step-by-step implementation plans that MESH was built from. They record *how* each feature was decided and aren't needed for using MESH.

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
