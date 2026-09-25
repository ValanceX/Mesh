# Editor setup

This guide sets up an editor for MPRX: the `mesh-lsp` language server for diagnostics, quick fixes, hover, go to definition and completion, and the grammar's Tree-sitter queries for syntax highlighting. It takes about fifteen minutes.

**You'll need:** a clone of this repo, Rust (as in [Getting started](./getting-started.md)), and Node.js 22 or newer, to build the Tree-sitter parser with the CLI version the grammar pins.

Everything the server says comes from the MESH compiler: its diagnostics are exactly `mesh check`'s, for the same file, manifest and component. The highlighting queries only colour text; nothing in MESH reads them.

## 1. Install the language server

With npm, on Linux x64 or arm64, macOS x64 or arm64, or Windows x64:

```console
$ npm install -g @valancex/mesh-lsp
```

Or from the repo, anywhere Rust runs:

```console
$ cargo install --path crates/mesh-lsp
```

Either puts `mesh-lsp` on your `PATH`: npm's starts the native server it installed for your platform, and `cargo install`'s is the server itself, in `~/.cargo/bin` beside `mesh`. Your editor starts it; you don't run it yourself.

## 2. Build the parser

Highlighting needs the MPRX grammar compiled for your machine. Build it with the grammar's own, pinned Tree-sitter CLI:

```console
$ cd grammar/tree-sitter-mprx
$ npm ci --ignore-scripts
$ npm rebuild tree-sitter-cli
$ npx tree-sitter build -o mprx.so
```

This builds `mprx.so`, a shared library for your platform. The query is `queries/highlights.scm` in the same directory.

## 3. Tell the server what to check

`mesh-lsp` checks each `.mprx` file against a component manifest, as `mesh check --model` does. It needs two settings, because it never guesses a component from a file's name:

- `model`: the manifest, relative to the workspace root;
- `components`: each document's path, relative to the workspace root, mapped to the component whose template it is.

A document that isn't in `components` is still checked for syntax and structure, as `mesh check` without `--model` checks it. See the [`mesh-lsp` manual](../manual/mesh-lsp.md#configuring) for the details.

Two things every editor must do:

- **Send `mprx` as the document's language.** The server checks only documents whose `languageId` is `mprx`. Most editors send the language or file type you configure for `.mprx` files.
- **Attach the server to the manifest's JSON file too,** if you want the manifest's own diagnostics and want unsaved edits to it to take effect. The server recognizes the manifest by its path and ignores every other JSON file.

## Neovim 0.11 or later

*Verified with Neovim 0.11.4, by [`editors/neovim/verify.sh`](../../editors/neovim/verify.sh).*

Install the parser and the query where Neovim finds them, with no plugin needed:

```console
$ mkdir -p ~/.local/share/nvim/site/parser ~/.local/share/nvim/site/queries/mprx
$ cp grammar/tree-sitter-mprx/mprx.so ~/.local/share/nvim/site/parser/
$ cp grammar/tree-sitter-mprx/queries/highlights.scm ~/.local/share/nvim/site/queries/mprx/
```

Then add this to your `init.lua`, with your own manifest and components. It's the same file as [`editors/neovim/init.lua`](../../editors/neovim/init.lua):

```lua
-- MESH in Neovim 0.11 or later: the mesh-lsp language server, and MPRX
-- highlighting from the grammar's Tree-sitter queries.
-- See docs/guides/editor-setup.md.

-- `.mprx` files are MPRX. Neovim sends the filetype as the `languageId`,
-- and mesh-lsp checks only documents whose `languageId` is `mprx`.
vim.filetype.add({ extension = { mprx = "mprx" } })

vim.lsp.config("mesh", {
  cmd = { "mesh-lsp" },
  -- JSON too, so the manifest gets its diagnostics and its unsaved edits
  -- are seen. Other JSON files are ignored by the server.
  filetypes = { "mprx", "json" },
  root_markers = { "components.json", ".git" },
  -- The `"mesh"` settings: the manifest, and each file's component.
  init_options = {
    model = "components.json",
    components = {
      ["users-page.mprx"] = "users-page",
      ["user-card.mprx"] = "user-card-example",
    },
  },
})
vim.lsp.enable("mesh")

-- Highlighting, once the parser and query are installed (see the guide).
vim.api.nvim_create_autocmd("FileType", {
  pattern = "mprx",
  callback = function(args)
    pcall(vim.treesitter.start, args.buf, "mprx")
  end,
})
```

Open `examples/users-page.mprx` from a clone of the repo to try it: hover `user.name`, go to the definition of `avatar`, or type `<` on a new line.

- `:checkhealth vim.lsp` shows whether `mesh-lsp` is attached, and `:lua vim.print(vim.lsp.get_clients({ name = "mesh" }))` its settings.
- The server's messages (a document with no configured component, a manifest it couldn't read) go to Neovim's LSP log, `:lua vim.cmd.tabnew(vim.lsp.get_log_path())`. Neovim keeps only warnings and errors unless you lower its level with `vim.lsp.log.set_level("info")`.
- **On Linux and BSD,** Neovim doesn't let servers watch files, so a manifest changed outside Neovim is reloaded when you open it in Neovim or change the settings. Editing it in Neovim takes effect as you type, everywhere.

To check this setup yourself, run `editors/neovim/verify.sh` from the repo. It builds everything into a temporary directory, leaves your own Neovim configuration alone, and checks diagnostics, hover, definition, completion, highlighting and manifest edits. To check an installed server instead of one built from the repo, point `MESH_LSP` at it: `MESH_LSP="$(npm prefix -g)/bin/mesh-lsp" editors/neovim/verify.sh` for npm's.

## Helix

*Not verified.* This configuration follows Helix's documentation, but hasn't been run. In `~/.config/helix/languages.toml`:

```toml
[language-server.mesh-lsp]
command = "mesh-lsp"
config = { model = "components.json", components = { "users-page.mprx" = "users-page" } }

[[language]]
name = "mprx"
scope = "source.mprx"
file-types = ["mprx"]
roots = ["components.json"]
language-servers = ["mesh-lsp"]
grammar = "mprx"

[[grammar]]
name = "mprx"
source = { path = "/path/to/Mesh/grammar/tree-sitter-mprx" }
```

Then run `hx --grammar build`, and copy `queries/highlights.scm` to `~/.config/helix/runtime/queries/mprx/`. Helix names some captures differently from the query, which uses nvim-treesitter's names: rename `@boolean` to `@constant.builtin.boolean`, `@variable.member` to `@variable.other.member`, and `@function.call` to `@function`.

## Other editors

Any editor that can start a language server over stdio and send it `initializationOptions` can use `mesh-lsp`: start `mesh-lsp` with no arguments for `.mprx` files (and the manifest), with the `"mesh"` settings above as the initialization options.

Zed and VS Code add languages through extensions, and MESH doesn't ship one yet. A future extension will only start `mesh-lsp` and pass these settings; all of MESH's behaviour stays in the server.
