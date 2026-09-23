# Diagnostics reference (v0.1)

This page lists every diagnostic MESH v0.1 can produce: what triggers it, how serious it is, and how to fix it. For the output format, see the [CLI manual](./mesh-cli.md#diagnostic-format).

There are two severities:

- **error**: the file isn't valid MPRX. `mesh check` exits `1`.
- **warning**: the file is valid, but something is probably a mistake. `mesh check` still exits `0`.

Every example below lives in `examples/fixtures/`, next to a `.stderr` file with its exact expected output, and the test suite checks those files.

---

## Errors

### `syntax error`

The file doesn't match the MPRX grammar.

```text
error: syntax error
 --> examples/fixtures/fail/unclosed-tag.mprx:1:1
  |
1 | <page
  | ^^^^^
```

**Common causes**

| Input | Problem |
|---|---|
| `<page` | Tag never finished |
| `<a />` followed by `<b />` | More than one root element |
| an empty file | No root element |
| `data={ k: "v" }` | Object literal needs double braces: `data={{ k: "v" }}` |
| `<text>a < b</text>` | `<` in text; write `{"a < b"}` |
| `data-id="1"` | Hyphen in an attribute name |
| `items[0]`, `user?.name`, `<!-- -->` | Not part of MPRX v0.1 |
| `f(a, b,)` | Trailing comma in command arguments |

> **Known v0.1 limitation.** A syntax error always spans the whole document, so it's reported at `1:1` and underlines the first line, wherever the real mistake is. When the file is more than one line, bisect it. Remove or comment out halves (in a copy; MPRX has no comments) until the error goes away, and compare against the table above. Pointing at the real location is the first follow-up planned after v0.1.

When there's a syntax error, MESH doesn't build the semantic model, so no other diagnostics are reported for that file.

### `mismatched closing tag: opened with "A", closed with "B"`

An element's closing tag names a different element than its opening tag.

```text
error: mismatched closing tag: opened with "title", closed with "heading"
 --> examples/fixtures/fail/mismatched-closing-tag.mprx:2:3
  |
2 |   <title>Users</heading>
  |   ^^^^^^^^^^^^^^^^^^^^^^
```

- The span covers the whole element, from its opening `<` to the end of its closing tag. For a multi-line element, only the first line is shown.
- The comparison is exact and case-sensitive: `<Text></text>` is a mismatch.
- This is reported during validation, not parsing. So other errors and warnings in the same file are still reported, and the element keeps its *opening* tag's name.

**Fix:** make the closing tag match the opening tag.

### `expected a single root element`

The parser found no root element. In practice, empty files and multiple roots are reported as `syntax error` instead, so you're unlikely to see this one.

### `failed to parse source`

The parser couldn't produce any tree at all. This is an internal safeguard, not something valid or invalid input is expected to trigger. If you hit it, please open an issue with the file.

### `could not read <file>: <reason>`

Printed by the CLI, not the compiler, when the file can't be read: it's missing, it's a directory, you don't have permission, or it isn't valid UTF-8. This is a single line with no source snippet. The exit status is `1`.

---

## Warnings

### `duplicate attribute "name": this occurrence is shadowed by a later one`

The same attribute appears more than once on one element. The **last** occurrence wins. Each earlier occurrence is dropped from the compiled output and gets its own warning.

```text
warning: duplicate attribute "class": this occurrence is shadowed by a later one
 --> examples/fixtures/pass/duplicate-attribute.mprx:1:6
  |
1 | <div class="a" class="b" />
  |      ^^^^^^^^^
```

**Fix:** delete the occurrence you don't mean.

### `duplicate event binding "name": this occurrence is shadowed by a later one`

The same event is bound more than once on one element (for example, two `on.click` bindings). As with attributes, the last binding wins and each earlier one gets a warning.

```text
warning: duplicate event binding "click": this occurrence is shadowed by a later one
 --> examples/fixtures/pass/duplicate-event-binding.mprx:1:9
  |
1 | <button on.click={save} on.click={cancel} />
  |         ^^^^^^^^^^^^^^^
```

**Fix:** keep one binding. If you want both effects, bind one command that does both.

---

## Ordering

Diagnostics are reported element by element, from the root downwards. Within each element they come in this order:

1. duplicate attributes
2. duplicate event bindings
3. mismatched closing tag
4. then the diagnostics of each child, in order

So a parent's mismatched-tag error comes before a warning inside one of its children. The `examples/fixtures/fail/warning-and-error.mprx` fixture shows a warning and an error in one file.

## What v0.1 doesn't diagnose

These pass `mesh check` in v0.1 even when they're wrong:

- References to names that don't exist (`user={usr}`)
- Unknown components or props
- Type mismatches (`disabled={"yes"}`)
- Unknown event names or commands

Catching them needs a component model. That's the next milestone after v0.1.
