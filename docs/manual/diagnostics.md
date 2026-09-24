# Diagnostics reference

This page lists every diagnostic MESH can produce: its code, what triggers it, how serious it is, and how to fix it. For the output format, see the [CLI manual](./mesh-cli.md#diagnostic-format).

Every diagnostic has a **code**, shown in brackets after its severity: `error[unterminated-tag]`. Codes are stable:

- A code is never renamed.
- A retired code is never reused for a different meaning.
- Messages, locations and wording may improve between versions; codes don't change meaning.
- `syntax-error` is the permanent fallback for a syntax problem MESH can't classify more precisely. A later version may give such a problem a more specific code.

There are two severities:

- **error**: the file isn't valid MPRX. `mesh check` exits `1`.
- **warning**: the file is valid, but something is probably a mistake. `mesh check` still exits `0`.

Every example below lives in `examples/fixtures/`, next to a `.stderr` file with its exact expected output, and the test suite checks those files.

---

## Syntax errors

Most syntax errors are reported where the mistake is; the exceptions are covered [below](#syntax-error). A file can have several, and each is reported separately, in source order:

```text
error[less-than-in-text]: `<` in text starts a tag; to show a literal `<`, put the text in a string expression, like `{"a < b"}`
 --> examples/fixtures/fail/several-syntax-errors.mprx:2:11
  |
2 |   <text>a < b</text>
  |           ^

error[hyphenated-attribute-name]: attribute name `data-id` can't contain `-`; use camelCase or `_` instead
 --> examples/fixtures/fail/several-syntax-errors.mprx:3:10
  |
3 |   <badge data-id="1" />
  |          ^^^^^^^

error[single-brace-object]: an object needs its own braces inside `{...}`: write `{{ key: value }}`
 --> examples/fixtures/fail/several-syntax-errors.mprx:4:15
  |
4 |   <list items={ first: 1 } />
  |               ^^^^^^^^^^^^
```

When a file has a syntax error, MESH doesn't build the semantic model, so no validation errors or warnings are reported for that file.

A single mistake can occasionally produce a second, follow-on syntax error nearby. Fix the first one and check again.

### `syntax-error`

The file doesn't match the MPRX grammar, and the problem isn't one of the more specific shapes below. The message says what MESH could tell:

| Message | Meaning |
|---|---|
| `expected a root element, but the file is empty` | The file is empty, or only whitespace. Reported at `1:1`. |
| `expected a single root element, found another one here` | A second top-level element. Points at the extra element. |
| `expected an expression` | An operator is missing its operand, as in `{count +}` or `{}`. Points just after the operator. |
| `invalid syntax` | Anything else. Points at the smallest region the parser could isolate. |
| `expected a single root element` | An internal safeguard. Valid or invalid input isn't expected to trigger it. |
| `failed to parse source` | An internal safeguard: the parser couldn't produce any tree at all. |

If you hit either safeguard, please open an issue with the file.

```text
error[syntax-error]: expected an expression
 --> examples/fixtures/fail/expected-expression.mprx:1:18
  |
1 | <counter>{count +}</counter>
  |                  ^
```

Some shapes that report `invalid syntax`:

| Input | Problem |
|---|---|
| `items[0]`, `user?.name`, `<!-- -->` | Not part of MPRX |
| `on:click={x}` | Event bindings use a dot: `on.click={x}` |
| `f(,)`, `f(a,,b)` | Empty command argument |
| `b=` with no value | Attribute value missing |
| `<page>` … `<p>x` … `</page>` | A nested element is never closed. When MESH can't tell which element is missing its closing tag, it reports the whole file. |
| `<p>hello < wörld</p>` | A `<` in text, where the parser can't isolate the mistake. Reports the whole file; see below. |

Sometimes the parser can't isolate a mistake and treats the whole file as one error. Then MESH reports `invalid syntax` for the whole file, pointing at `1:1`, rather than guessing a location. Which inputs do this depends on the exact text, including non-ASCII letters. For example, `<p>Größe < 10</p>` and `<p>héllo < world</p>` both get a located `less-than-in-text` error, but `<p>hello < wörld</p>` gets a whole-file `syntax-error`. Either way, the file is rejected. The fix is the same as for the specific error, if you can spot it; otherwise, look for the shapes in the table above.

### `unterminated-tag`

A tag was opened but never finished with `>` or `/>`. The span covers the `<` and the tag name.

```text
error[unterminated-tag]: unterminated tag `<badge`: expected `>` or `/>`
 --> examples/fixtures/fail/unterminated-nested-tag.mprx:3:3
  |
3 |   <badge count={3}
  |   ^^^^^^
```

A `<` followed straight by a name, with no space, starts a tag even in text: `a<b` reports an unterminated `<b` tag.

**Fix:** finish the tag with `>` (and add its closing tag) or `/>`.

### `missing-closing-tag`

The root element is opened with `<name>` but the file ends without its `</name>`. The span covers the opening tag.

```text
error[missing-closing-tag]: `<card>` is never closed: expected `</card>`
 --> examples/fixtures/fail/missing-closing-tag.mprx:1:1
  |
1 | <card>
  | ^^^^^^
```

While the root element is unclosed, MESH can't tell what's inside it, so this is the only error reported for that element. Other mistakes inside it, such as a `<` in text or a hyphenated attribute name, show up once you add the closing tag.

**Fix:** add the closing tag, or write the element as self-closing: `<card />`.

### `less-than-in-text`

Text contains a `<` used as a comparison, such as `a < b`, `a <= b` or `x<5`. In MPRX, `<` in text always starts a tag.

```text
error[less-than-in-text]: `<` in text starts a tag; to show a literal `<`, put the text in a string expression, like `{"a < b"}`
 --> examples/fixtures/fail/less-than-in-text.mprx:2:15
  |
2 |   <text>Total < 10</text>
  |               ^
```

**Fix:** put the text in a string expression: `<text>{"Total < 10"}</text>`.

### `hyphenated-attribute-name`

An attribute name contains `-`, as in `data-id="1"`. MPRX attribute names are identifiers. The span covers the whole name.

```text
error[hyphenated-attribute-name]: attribute name `data-id` can't contain `-`; use camelCase or `_` instead
 --> examples/fixtures/fail/hyphenated-attribute-name.mprx:1:7
  |
1 | <card data-id="1" />
  |       ^^^^^^^
```

Tag names can contain `-` (`<my-card>`); attribute names can't.

If the name also has a character that no name can contain, as in `é-id`, you get a `syntax-error` at that character instead: this code would cover only part of the mistake.

**Fix:** rename the attribute: `dataId="1"` or `data_id="1"`.

### `single-brace-object`

An object literal was written inside a single `{...}`, as in `data={ k: "v" }`. The outer braces hold an expression; the object needs its own. The span covers the braces and their contents.

```text
error[single-brace-object]: an object needs its own braces inside `{...}`: write `{{ key: value }}`
 --> examples/fixtures/fail/single-brace-object.mprx:1:12
  |
1 | <page data={ k: "v" } />
  |            ^^^^^^^^^^
```

**Fix:** double the braces: `data={{ k: "v" }}`.

### `command-trailing-comma`

A command's argument list ends with a comma, as in `save(user, draft,)`. Array literals allow a trailing comma; command arguments don't.

```text
error[command-trailing-comma]: trailing comma in command arguments; remove the `,`
 --> examples/fixtures/fail/command-trailing-comma.mprx:1:35
  |
1 | <button on.click={save(user, draft,)} />
  |                                   ^
```

**Fix:** remove the trailing comma.

### `malformed-event-binding`

An `on.` event binding doesn't have the shape `on.<event>={handler}`: the event name is missing (`on.={x}`), dotted (`on.click.once={x}`) or hyphenated (`on.my-event={x}`), or the value isn't in braces (`on.click="x"`, or no value at all). The span covers `on.` and the name.

```text
error[malformed-event-binding]: malformed event binding `on.click.once`: expected `on.<event>={handler}`
 --> examples/fixtures/fail/malformed-event-binding-dotted-name.mprx:1:9
  |
1 | <button on.click.once={save} />
  |         ^^^^^^^^^^^^^
```

When one element has several malformed bindings, the parser may recover them as one region; only the first is reported.

**Fix:** use a single identifier as the event name, and put the handler in braces: `on.click={save}`.

---

## Validation errors

### `mismatched-closing-tag`

An element's closing tag names a different element than its opening tag.

```text
error[mismatched-closing-tag]: mismatched closing tag: opened with "title", closed with "heading"
 --> examples/fixtures/fail/mismatched-closing-tag.mprx:2:3
  |
2 |   <title>Users</heading>
  |   ^^^^^^^^^^^^^^^^^^^^^^
```

- The span covers the whole element, from its opening `<` to the end of its closing tag. For a multi-line element, only the first line is shown.
- The comparison is exact and case-sensitive: `<Text></text>` is a mismatch.
- This is reported during validation, not parsing. So other errors and warnings in the same file are still reported, and the element keeps its *opening* tag's name.

**Fix:** make the closing tag match the opening tag.

---

## Warnings

### `duplicate-attribute`

The same attribute appears more than once on one element. The **last** occurrence wins. Each earlier occurrence is dropped from the compiled output and gets its own warning.

```text
warning[duplicate-attribute]: duplicate attribute "class": this occurrence is shadowed by a later one
 --> examples/fixtures/pass/duplicate-attribute.mprx:1:6
  |
1 | <div class="a" class="b" />
  |      ^^^^^^^^^
```

**Fix:** delete the occurrence you don't mean.

### `duplicate-event-binding`

The same event is bound more than once on one element (for example, two `on.click` bindings). As with attributes, the last binding wins and each earlier one gets a warning.

```text
warning[duplicate-event-binding]: duplicate event binding "click": this occurrence is shadowed by a later one
 --> examples/fixtures/pass/duplicate-event-binding.mprx:1:9
  |
1 | <button on.click={save} on.click={cancel} />
  |         ^^^^^^^^^^^^^^^
```

**Fix:** keep one binding. If you want both effects, bind one command that does both.

---

## Manifest errors

`mesh check --model <FILE>` loads a component manifest before it reads the `.mprx` file. It checks the whole manifest first. If the manifest has any errors, `mesh check` reports all of them, in source order, and nothing else: it doesn't check the `.mprx` file at all. The exit status is `1`.

Manifest errors point into the manifest, with the manifest's path, line and column. Every code starts with `manifest-`. The manifest's format is published as a JSON Schema in [`schemas/manifest-v1.schema.json`](../../schemas/manifest-v1.schema.json).

### `manifest-syntax-error`

The manifest isn't valid JSON. The message is the JSON parser's, and the location is where it stopped.

Only the first JSON syntax error is reported. A leading byte-order mark is allowed.

**Fix:** correct the JSON. Trailing commas and comments aren't allowed.

### `manifest-unsupported-version`

The manifest's `"version"` is missing, or isn't `1`, the only version this MESH reads. Another version has another format, so nothing else in the manifest is checked.

**Fix:** write `"version": 1`, and the rest of the manifest in version 1's format.

### `manifest-invalid-value`

A value has the wrong JSON type: an array where an object belongs, a string for `"required"`, a type that isn't an object, and so on.

**Fix:** use the JSON type the message names.

### `manifest-missing-property`

An object is missing a property it must have. Points at the object's opening `{`.

- The manifest needs `"version"`, `"types"` and `"components"`.
- A component needs all of `"props"`, `"events"`, `"commands"` and `"scope"`, even when they're empty.
- A prop or record field needs both `"type"` and `"required"`. `"required"` has no default.
- A type needs a `"kind"`, plus `"element"` for a list, `"fields"` for a record, `"name"` for a named type, and `"type"` for an optional type.

**Fix:** add the property.

### `manifest-unknown-property`

An object has a property its format doesn't allow. Points at the property's key.

**Fix:** remove the property. The manifest describes interfaces only: there are no default values, and a type has only the properties of its kind.

### `manifest-duplicate-key`

A key appears twice in one object. Most JSON tools silently keep the last one; MESH reports the repeat instead, and uses the first occurrence. Points at the second occurrence.

This covers every level: component names, props, events, commands, scope names, named types, record fields, and properties such as `"type"`.

**Fix:** delete or rename one of them.

### `manifest-unknown-kind`

A type's `"kind"` isn't one of `string`, `number`, `boolean`, `null`, `any`, `list`, `record`, `named` or `optional`.

`void` and `nothing` are types MESH uses internally, and a manifest can't write them either.

**Fix:** use one of the kinds above. For a number, use `number`: there's no separate integer type.

### `manifest-invalid-name`

A declared name can't be written in MPRX where it would be used.

- A component name must be a valid tag name: a letter or `_`, then letters, digits, `_` or `-`.
- Every other name (props, events, commands, command parameters, scope names, named types, record fields) must be a valid identifier: a letter or `_`, then letters, digits or `_`. No `-`, because MPRX reads `-` as minus.
- None of those identifiers can be `true`, `false` or `null`. MPRX's lexer always reads those words as literals, so a prop, scope name or field spelled that way could never be referred to.

**Fix:** rename it, for example `aria-label` to `ariaLabel`.

### `manifest-duplicate-parameter`

A command declares two parameters with the same name. Points at the second one's name.

**Fix:** rename one of them.

### `manifest-unknown-type`

A `named` type refers to a name that `"types"` doesn't declare. Points at the name.

**Fix:** declare the type in `"types"`, or correct the name. Names are case-sensitive.

### `manifest-recursive-type`

A named type refers to itself, directly or through other named types. Each cycle is reported once, at the first of its types in the manifest.

Recursive types aren't supported in version 1.

**Fix:** break the cycle, for example by typing the recursive part `any`.

### `manifest-nested-optional`

An `optional` type wraps a type that is already optional: another `optional`, or a named type that is one. Points at the inner type.

**Fix:** remove one of the two. "Optional optional" means the same as "optional".

### `manifest-missing-component`

The manifest loaded, but it doesn't declare the component the file is the template of. That component is the file's name without its extension (`users-page.mprx` is `users-page`), unless `--component` names another. Points at the manifest's `"components"` key.

**Fix:** declare the component, or pass `--component <NAME>`.

---

## CLI errors

### `could not read <file>: <reason>`

Printed by the CLI, not the compiler, when the file (or the `--model` manifest) can't be read: it's missing, it's a directory, you don't have permission, or it isn't valid UTF-8. This is a single line with no code and no source snippet. The exit status is `1`.

---

## Ordering

Syntax errors are reported in source order.

Validation diagnostics are reported element by element, from the root downwards. Within each element they come in this order:

1. duplicate attributes
2. duplicate event bindings
3. mismatched closing tag
4. then the diagnostics of each child, in order

So a parent's mismatched-tag error comes before a warning inside one of its children. The `examples/fixtures/fail/warning-and-error.mprx` fixture shows a warning and an error in one file.

## What MESH doesn't diagnose yet

These pass `mesh check` even when they're wrong:

- References to names that don't exist (`user={usr}`)
- Unknown components or props
- Type mismatches (`disabled={"yes"}`)
- Unknown event names or commands

Catching them needs a component model. That's the rest of v0.2.
