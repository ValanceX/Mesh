# `mesh-lsp` manual

`mesh-lsp` is the MESH language server. It speaks the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) over stdio, so any editor with an LSP client can use it.

It is a client of the MESH compiler, not a second implementation of it. The diagnostics it shows are exactly the ones `mesh check` reports for the same text, manifest and component: the same codes, messages, locations and suggestions. A test compares the two on every fixture in the repository.

What it does today:

- **Diagnostics** for `.mprx` documents, and for the component manifest itself, kept current as you type.
- **Quick fixes** from suggestions: `did you mean "user"?` becomes a one-click replacement.
- **Hover:** the type of the expression under the cursor, and what a name is declared as in the manifest.
- **Go to definition:** from a tag, prop, event, command or scope name to where the manifest declares it.
- **Completion** of tags, props, events, scope names, record members and commands, from the manifest.

Syntax highlighting comes from the grammar's Tree-sitter queries, not from the server. The [editor setup guide](../guides/editor-setup.md) sets up both.

## Installing

With npm, no Rust needed:

```console
$ npm install -g @valancex/mesh-lsp
```

This puts `mesh-lsp` on your `PATH`. npm installs the native server for your platform, one of Linux x64 and arm64, macOS x64 and arm64, and Windows x64, and `mesh-lsp` starts it, changing nothing the editor sees. Don't pass `--omit=optional`: the binary is an optional dependency.

Anywhere else, or from a clone of the repo:

```console
$ cargo install --path crates/mesh-lsp
```

This installs a binary named `mesh-lsp` into `~/.cargo/bin`. Either way, your editor starts it; you don't run it yourself.

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

The server reads the settings from the client's `initializationOptions`. If the client supports `workspace/configuration`, the server also asks it for section `"mesh"` at startup; the answer replaces `initializationOptions` unless it's empty. When the settings change (`workspace/didChangeConfiguration`), it asks again if the client supports that, and otherwise reads `settings.mesh` from the notification. A setting of the wrong type is logged and left out; it never stops the server.

The [editor setup guide](../guides/editor-setup.md) has a complete configuration for Neovim, checked in a running Neovim, and an unverified one for Helix.

### Which files it checks

- **MPRX documents** are the ones the client opens with `languageId` `mprx`. Configure your editor to send that for `.mprx` files; the server doesn't look at file extensions. Any other document is ignored: nothing is checked or published for it.
- **The manifest** is the file `model` names, recognized by its path whatever its `languageId`. Attach the server to it (usually to JSON files) to see its diagnostics and to have unsaved edits to it take effect.

What each open file is gets decided again whenever the settings change: a manifest you opened before configuring `model` becomes the manifest, and one that `model` no longer names stops being it.

## What it publishes, and when

**Documents.** Opening a document checks it at once. After a change, it's checked again 150 ms after your last keystroke. Each result is published with the version of the text it was computed for. A result for text you've since changed is never published. Closing a document clears its diagnostics.

**The manifest.** The manifest is read from the editor while it's open there, and from disk otherwise. Each version is loaded once. When it changes, every open document is checked again against the new version. That happens when you edit it, close it (back to disk), change it on disk, or change the settings.

- While the manifest is valid, a document is checked against it, as `mesh check --model <manifest> --component <component>` would check it.
- While the manifest has errors, they're published on the manifest file, and documents are checked without a model, as `mesh check` without `--model` would check them. (`mesh check` itself reports only the manifest's errors and stops; in an editor you still want your template's own syntax errors while you fix the manifest.)
- If a document's configured component isn't in the manifest, `manifest-missing-component` is published on the manifest, and that document is checked without a model.

**Quick fixes.** Each suggestion is offered as a `quickfix` code action that replaces the suggestion's range. It's offered only for the version of the text the diagnostics were computed for, so an edit never lands in the wrong place.

## Hover and go to definition

Both answer from the same check that produced the diagnostics, so they need a model: a document checked without one (no `model` setting, a manifest with errors, or no entry in `components`) has no hover and no definitions.

**On a file with a syntax error** they still work on the parts that parse. An element counts once its tag name is written, even if the rest of its tag isn't finished yet; what it keeps is its attributes, event bindings and children that parse. A `{…}` block that doesn't parse is left out, but a complete expression inside it still counts: in `{user.}`, `user` does. One unclosed `{` can make the whole rest of the file unreadable to the parser, and then the complete elements before it still work. The diagnostics are unaffected: they are always exactly `mesh check`'s, whatever hover can still see.

**Hover** shows what the cursor is on:

- **A tag name:** the component, with each prop and its type (`size?: string` marks a prop that isn't required) and each event (`on.click: any`, or `on.close` if it carries no value).
- **A prop or event name:** its type, and whether the prop is required.
- **A scope name**, such as `user`: its type, and what a named type is defined as (`type User = { active: boolean, avatar?: string, name: string }`).
- **A command:** its parameters, such as `selectUser(user: User)`.
- **Any other expression**, such as the `name` of `user.name`: its type, such as `string`. An expression that has a mistake in it has no type, so it has no hover.

Types are written the way diagnostics write them. Hovers are Markdown if the client accepts it, and plain text otherwise.

**Go to definition** jumps to the name's key in the manifest, such as `"avatar"` under `"components"`. If the manifest is open in the editor, that's its current text.

## Completion

Completion offers only names the manifest declares, and each only where the compiler accepts it:

- **After `<`:** every component.
- **In an opening tag,** after its name or a whole attribute: the component's props, required ones first (marked *required*), then the rest, then its events as `on.click`. After `on.`, just its events.
- **At the start of an `on.` handler:** the template's commands. A handler must be a command, so nothing else is offered there.
- **Where any other expression starts:** the names in the template's scope.
- **After `.`:** the fields of the value's type, if it's a record. A value that may be absent has none (reading through it is `possibly-absent-access`), and neither does `any` or a value whose type isn't known, such as a name that isn't in scope.

Everything else is in name order. It works while the file is broken, which it almost always is while you type: after `user.`, after `<avatar `, after a bare `<`. Like hover, it needs a model; without one it offers nothing. It doesn't complete attribute values, text inside strings, closing tags, object keys, `$event`, or argument lists.

## Requests right after typing

A hover, definition or completion request made before the new text has been checked waits for that check, which then runs at once instead of after the usual 150 ms. If you change the text again before it's answered, it's answered with `ContentModified`, which editors treat as "ask again".

## Positions

Positions are counted in the unit the client prefers: UTF-8 if it offers that, then UTF-32, and UTF-16 otherwise.

## Exit status

`0` if the client sent `shutdown` and then `exit`, as the protocol requires; `1` otherwise, including when stdin or stdout closes. The server stops cleanly in each case; it never panics on input.

A message that isn't JSON-RPC at all (not JSON, or JSON without a method or id) also stops the server with status `1`. The transport can't read past it, and without an id there's no request to answer. A well-formed request whose parameters are wrong gets an `InvalidParams` error, and the server keeps running.

## Limitations

- **One workspace root:** the first workspace folder, or `rootUri`. Multi-root workspaces aren't supported.
- **Paths compare exactly.** On a case-insensitive file system, write `components` keys with the same case the editor uses.
- **Disk changes to the manifest** are seen automatically only if the client supports registering file watchers dynamically (Neovim doesn't on Linux). Otherwise they're seen when you open the manifest in the editor or change the settings.
- **Closing tags and members have no definition.** Go to definition works from an opening tag's name, not a closing tag's, and from `user` in `user.name` but not from `name`, which hover still shows the type of.
- **A lone `\r`** (a classic Mac line ending) isn't a line break to MESH, so positions in such a file disagree with the editor's.
