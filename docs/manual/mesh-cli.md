# `mesh` CLI manual (v0.1)

`mesh` is the native command-line tool for MESH. In v0.1 it has one command, `check`.

## Synopsis

```text
mesh check [--model <MANIFEST> [--component <NAME>]] <FILE>
mesh help [COMMAND]
mesh --help
mesh --version
```

## Installing

From a clone of the repo:

```console
$ cargo install --path crates/mesh-cli
```

This installs a binary named `mesh` into `~/.cargo/bin`. You can also run it without installing, from the repo root, with `cargo run -p mesh-cli -- <args>`. See [Getting started](../guides/getting-started.md) for the full walkthrough.

## `mesh check`

Parses and validates one MPRX file, then reports every diagnostic.

**Arguments and options**

| | Meaning |
|---|---|
| `<FILE>` | Path to the `.mprx` file. Any path works, and the extension isn't enforced. The file must be valid UTF-8. |
| `--model <MANIFEST>` | A component manifest (JSON) declaring the components the file uses. See [Checking against a manifest](#checking-against-a-manifest). |
| `--component <NAME>` | The manifest component whose template the file is. Defaults to the file's name without its extension. Needs `--model`. |

**What it checks**

1. **Syntax**: the file is one well-formed MPRX element tree (see the [language spec](../MPRX-SPEC.md)).
2. **Structure**: closing tags match opening tags, and no element repeats an attribute or event binding.

Without `--model`, that's all it checks. References, component names, props and events are checked only against a component manifest.

### Checking against a manifest

With `--model`, `mesh check` first loads the manifest and checks all of it: the JSON, its `"version"`, its format, and its declarations. The format is published as a JSON Schema in [`schemas/manifest-v1.schema.json`](../../schemas/manifest-v1.schema.json), and [`examples/components.json`](../../examples/components.json) is a complete example. If the manifest has any errors, `mesh check` reports every one, with the manifest's path, line and column, and stops without reading `<FILE>`. Their codes all start with `manifest-`; see the [diagnostics reference](./diagnostics.md#manifest-errors).

Every `.mprx` file is the template of one component in the manifest. By default that is the file's name without its extension, so `users-page.mprx` is the template of `users-page`. `--component` names another. A name the manifest doesn't declare is a `manifest-missing-component` error.

```console
$ mesh check --model examples/components.json examples/users-page.mprx
no errors
$ mesh check --model examples/components.json --component user-card-example examples/user-card.mprx
no errors
```

With a valid manifest, `mesh check` checks the file as that component's template, after the checks above:

1. **Components**: every element's tag is a component the manifest declares. There are no built-in elements.
2. **Props and events**: every attribute is a prop of the element's component, every required prop is supplied, and every `on.<name>` is one of its events.
3. **Names**: every reference is in the template's `scope`, and every command is one of the template's `commands`, with the right number of arguments.
4. **Placement**: a command appears only as the whole handler of an `on.` binding, and `$event` only in that command's arguments, for an event that carries a value.

These are errors with their own codes, listed under [Model errors](./diagnostics.md#model-errors), and a close match gets a `help` line. Types aren't checked yet: that comes later in v0.2.

### Output

MESH keeps its two output streams separate, so scripts can rely on them:

| Stream | What's written |
|---|---|
| **stderr** | Every diagnostic, in the order the compiler produced it, each followed by one blank line. Also used for "could not read" failures. |
| **stdout** | The single line `no errors` if the file has no *errors*. Warnings alone still print it. Nothing otherwise. |

Diagnostics come out in source order within each element: first the element's attributes, then its event bindings, then its closing tag, then its children. Their order is never re-sorted.

### Diagnostic format

Each diagnostic is one block:

```text
<severity>[<code>]: <message>
 --> <file>:<line>:<column>
  |
<N> | <source line>
  | <carets>
  = help: did you mean "<name>"?
```

For example:

```text
error[mismatched-closing-tag]: mismatched closing tag: opened with "title", closed with "heading"
 --> examples/fixtures/fail/mismatched-closing-tag.mprx:2:3
  |
2 |   <title>Users</heading>
  |   ^^^^^^^^^^^^^^^^^^^^^^
```

The exact rules:

- **Severity** is `error` or `warning`.
- **Code** is the diagnostic's stable code, such as `mismatched-closing-tag`. A code is never renamed or reused for a different meaning, so scripts can match on it; messages may change. The [diagnostics reference](./diagnostics.md) lists every code.
- **`<file>`** is the path exactly as you passed it on the command line.
- **Line and column** are 1-based. Columns count characters (Unicode scalar values), not bytes and not display width. A tab counts as one column.
- **The gutter** (the spaces before `|`) widens to fit the line number, so line 120 gets a three-character gutter.
- **Carets** underline the problem's span on the reported line. If a span covers several lines, only its first line is shown and underlined up to the end of that line. An empty span gets a single `^`.
- **Tabs** in the source line are kept in the caret line's indentation, so carets line up in your terminal. Wide characters (CJK, emoji) may make the carets look shifted, because v0.1 doesn't measure display width.
- **Windows line endings** (`\r\n`) are handled; the `\r` is never echoed or counted.
- **`= help:` lines** follow the carets when MESH has a suggestion, one line per suggestion. Most diagnostics have none. Today every suggestion is a similarly spelled name that the manifest declares.
- **A leading UTF-8 byte-order mark** is skipped: it isn't echoed or counted, so line 1's columns match what your editor shows.
- The output has no colour, and it isn't machine-readable yet.

The full list of codes and messages is in the [diagnostics reference](./diagnostics.md).

### Exit status

| Status | Meaning |
|---|---|
| `0` | No errors. Warnings may have been printed. `no errors` is on stdout. |
| `1` | At least one error diagnostic (in the file or the manifest), or the file or manifest couldn't be read (missing, a directory, not UTF-8, permission denied). |
| `2` | Usage error, such as a missing `<FILE>` argument, an unknown command or flag, or `--component` without `--model`. Reported by the argument parser. |

An unreadable file prints a single line, with no source snippet:

```console
$ mesh check nope.mprx
error: could not read nope.mprx: No such file or directory (os error 2)
```

### Using it in scripts and CI

Because warnings exit `0` and errors exit `1`, `mesh check` works directly as a gate:

```sh
status=0
for f in ui/*.mprx; do
  mesh check "$f" || status=1
done
exit $status
```

To fail on warnings as well, test stderr:

```sh
out=$(mesh check "$f" 2>&1 >/dev/null)
[ -z "$out" ] || { printf '%s\n' "$out"; exit 1; }
```

## `mesh help`, `--help` and `--version`

`mesh help` and `--help` print usage for `mesh` or for a single command. `mesh --version` (or `-V`) prints the version, such as `mesh 0.1.0`, and exits `0`.

## Limitations in v0.1

- **One file per invocation.** To check several files, loop over them.
- **No `compile` command yet.** The compiler produces the Semantic IR, but the CLI doesn't write it out. Use the [Rust API](../guides/embedding-the-compiler.md) to get at it.
