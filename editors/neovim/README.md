# Neovim

`init.lua` is the configuration in [the editor setup guide](../../docs/guides/editor-setup.md#neovim-011-or-later); a test keeps the two identical.

`verify.sh` checks that configuration in a real, headless Neovim 0.11 or later. It builds `mesh-lsp` and the MPRX parser, installs them in a temporary directory (never your own Neovim's), copies `examples/` there, and runs `verify.lua`, which checks diagnostics, hover, go to definition, completion on broken text, highlighting, and edits to the manifest. It prints one line per check and exits non-zero if any fails.

```console
$ editors/neovim/verify.sh
$ NVIM=/path/to/nvim editors/neovim/verify.sh
```

It needs Rust, Node.js (for the grammar's pinned Tree-sitter CLI; run `npm ci --ignore-scripts && npm rebuild tree-sitter-cli` in `grammar/tree-sitter-mprx` first) and `git`. It isn't run in CI.
