# `mesh-lsp` manual

`mesh-lsp` is the MESH language server. It speaks the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) over stdio, so any editor with an LSP client can use it.

It is a client of the MESH compiler, not a second implementation of it. The diagnostics it shows are exactly the ones `mesh check` reports for the same text, manifest and component: the same codes, messages, locations and suggestions. A test compares the two on every fixture in the repository.

What it does today:

- **Diagnostics** for `.mprx` documents, and for the component manifest itself, kept current as you type.
- **Quick fixes** from suggestions: `did you mean "user"?` becomes a one-click replacement.

Hover, go-to-definition, completion, and highlighting queries for the grammar are coming in later v0.3 passes.

## Installing

From a clone of the repo:

```console
$ cargo install --path crates/mesh-lsp
```

This installs a binary named `mesh-lsp` into `~/.cargo/bin`. Your editor starts it; you don't run it yourself.

## Configuring

The server needs two answers that `mesh check` takes as flags: which manifest to check against, and which component each file is the template of. It never guesses a component from a file's name. A name isn't a stable identity in an editor: an unsaved document has none, and renaming a file would silently change what it means. So you name each file's component explicitly.

The settings are one object, the `"mesh"` section:

```json
{
  "model": "components.json",
  "components": {
    "users-page.mprx": "users-page",
    "user-card.mprx": "user-card-example"
  }
}
```

| Setting | Meaning |
|---|---|
| `model` | The manifest, as a path relative to the workspace root (or absolute). Without it, every document is checked without a model, as `mesh check` without `--model` checks it. |
| `components` | Each document's path, relative to the workspace root and written with `/`, mapped to the component whose template it is. A document the map doesn't name is checked without a model, and the server logs that once. |

The server reads the settings from the client's `initializationOptions`. When they change (`workspace/didChangeConfiguration`), it asks the client for section `"mesh"` if the client supports `workspace/configuration`, and otherwise reads `settings.mesh` from the notification. A setting of the wrong type is logged and left out; it never stops the server.

For example, in Neovim 0.10 or later:

```lua
vim.filetype.add({ extension = { mprx = "mprx" } })
vim.api.nvim_create_autocmd("FileType", {
  pattern = "mprx",
  callback = function(args)
    vim.lsp.start({
      name = "mesh-lsp",
      cmd = { "mesh-lsp" },
      root_dir = vim.fs.root(args.buf, { "components.json", ".git" }),
      init_options = {
        model = "components.json",
        components = { ["users-page.mprx"] = "users-page" },
      },
    })
  end,
})
```

## What it publishes, and when

**Documents.** Opening a document checks it at once. After a change, it's checked again 150 ms after your last keystroke. Each result is published with the version of the text it was computed for. A result for text you've since changed is never published. Closing a document clears its diagnostics.

**The manifest.** The manifest is read from the editor while it's open there, and from disk otherwise. Each version is loaded once. When it changes, every open document is checked again against the new version. That happens when you edit it, close it (back to disk), change it on disk, or change the settings.

- While the manifest is valid, a document is checked against it, as `mesh check --model <manifest> --component <component>` would check it.
- While the manifest has errors, they're published on the manifest file, and documents are checked without a model, as `mesh check` without `--model` would check them. (`mesh check` itself reports only the manifest's errors and stops; in an editor you still want your template's own syntax errors while you fix the manifest.)
- If a document's configured component isn't in the manifest, `manifest-missing-component` is published on the manifest, and that document is checked without a model.

**Quick fixes.** Each suggestion is offered as a `quickfix` code action that replaces the suggestion's range. It's offered only for the version of the text the diagnostics were computed for, so an edit never lands in the wrong place.

**Positions** are counted in the unit the client prefers: UTF-8 if it offers that, then UTF-32, and UTF-16 otherwise.

## Exit status

`0` if the client sent `shutdown` and then `exit`, as the protocol requires; `1` otherwise, including when stdin or stdout closes, or a message isn't valid JSON-RPC. The server stops cleanly in each case; it never panics on input.

## Limitations

- **One workspace root:** the first workspace folder, or `rootUri`. Multi-root workspaces aren't supported.
- **Paths compare exactly.** On a case-insensitive file system, write `components` keys with the same case the editor uses.
- **Disk changes to the manifest** are seen automatically only if the client supports registering file watchers dynamically. Otherwise they're seen when you reopen the manifest or change the settings.
- **A lone `\r`** (a classic Mac line ending) isn't a line break to MESH, so positions in such a file disagree with the editor's.
