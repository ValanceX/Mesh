# @valancex/mesh-lsp

Installs and starts [`mesh-lsp`](https://github.com/ValanceX/Mesh), the MESH language server for MPRX, with no Rust toolchain.

```console
$ npm install -g @valancex/mesh-lsp
$ mesh-lsp        # what your editor runs; it speaks LSP over stdio
```

The server is a native binary. npm installs the one for your platform, from one of these packages, and `mesh-lsp` starts it:

| Platform | Package |
|---|---|
| Linux x64 | `@valancex/mesh-lsp-linux-x64` |
| Linux arm64 | `@valancex/mesh-lsp-linux-arm64` |
| macOS x64 | `@valancex/mesh-lsp-darwin-x64` |
| macOS arm64 | `@valancex/mesh-lsp-darwin-arm64` |
| Windows x64 | `@valancex/mesh-lsp-win32-x64` |

The Linux binaries are static, so they run on any distribution. Everything the server does is in the binary: this package only starts it, passing its arguments and streams through unchanged, and exits as the server exits. Don't install with `--omit=optional`, which skips the binary.

Elsewhere, build the server from source: `cargo install --path crates/mesh-lsp` in a clone of the repository.

To set up your editor, see the [editor setup guide](https://github.com/ValanceX/Mesh/blob/main/docs/guides/editor-setup.md).

## License

MIT
