# Diagnostics reference

This page lists every diagnostic MESH can produce: its code, what triggers it, how serious it is, and how to fix it. For the output formats, see the CLI manual: [human](./mesh-cli.md#diagnostic-format) and [JSON](./mesh-cli.md#json-output).

Runtime diagnostics (the `assembly-*` and `runtime-*` codes) may also carry a **`hint`**, the corrective action, for the codes where one reliably applies; see [the runtime manual](./runtime.md#the-codes).

Every diagnostic has a **code**, shown in brackets after its severity, `error[unterminated-tag]`, and as the `code` property in JSON output. Codes are machine-readable API, so programs should match on them, never on messages. They are stable:

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
| `` `{#if}` is never closed: expected `{/if}` `` (or `{#each}`, `{/each}`) | A [block](templates.md#blocks) opened and not closed, or still open where the element it is in ends. Points at the opener. |
| `` `{/if}` with no open `{#if}` `` (or `{/each}`, `{:else}`, `{:else if}`) | A closer or branch with no open block of its kind. Points at it. |
| `` `{/each}` closes a block that is not open `` | A closer for the other kind of block than the one open. |
| `` malformed `{#if}` `` / `` malformed `{#each}` `` / `` malformed `{:else}` `` | A block opener that is not well formed: no condition, no list, no `as` and name, an empty key. The message says which and what to write. |
| `` `{:else if}` after `{:else}` `` | `{:else}` is the last branch of an `{#if}`. |
| `` unknown block `{#when` `` | The blocks are `{#if}` and `{#each}`. |
| `a block cannot be the document: put it inside an element` | A document is one element, so an `{#if}`, `{#each}` or `{#snippet}` sits inside an element. Points at the opener. (A `{@render}` may be the document, alone: nothing may stand beside it, and then the error is `invalid syntax`.) |
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

### `nesting-too-deep`

The file is valid MPRX, but it nests more than 128 levels deep, the deepest MESH supports. MPRX itself sets no limit: this one is MESH's, and a later version may raise it. Every element counts as a level, and so does every expression inside another one and every pair of parentheses, so `<a><b x={-(y)} /></a>` is 5 levels deep at `y`. A chain of operators nests too: in `a + b + c`, `a` is 3 expressions deep. The limit keeps MESH from running out of stack on a pathological file, which would crash it instead of reporting an error.

```text
error[nesting-too-deep]: this is nested more than 128 levels deep, the deepest MESH supports
   --> examples/fixtures/fail/nesting-too-deep.mprx:129:2
    |
129 | <box>
    |  ^^^
```

Only the first place that's too deep is reported, at the tag name of an element or at the whole expression. As with a syntax error, MESH doesn't build the semantic model, so nothing else is reported for the file.

**Fix:** flatten the file. No UI needs to nest this deeply; a file that does was almost certainly generated by mistake.

### `input-too-large`

The source is longer than 64 KB (65,536 bytes). MPRX itself sets no limit: this one is MESH's, and a later version may change it. It is checked before anything reads the file, so nothing else is reported, and the error is at the first byte past the limit. The limit bounds the work a checker can be given, which matters most to a host that checks text it did not write.

**Fix:** split the file into components. A view this long is easier to read in parts.

### `too-many-elements`

The source has more than 5,000 elements, counting every element in the file, nested or not. Like `input-too-large` it is MESH's limit, not MPRX's. Only the first element past the limit is reported, at its tag name, and, as with a syntax error, nothing else is reported for the file.

**Fix:** split the file into components. A list of items belongs in `{#each}`, which is one element however many items it renders.

### `too-many-attributes`

An element has more than 64 attributes, counting each event binding (`on.tap={…}`) as one. It is MESH's limit, not MPRX's. Only the first attribute past the limit is reported, at the attribute, and nothing else is reported for the file.

**Fix:** pass related values as one record in a single prop, or split the element into components.

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

### `number-literal-out-of-range`

A number literal so large that its nearest binary64 value would be infinite: more than about `1.8e308`, which takes 309 digits (spec §9.7.3). Points at the literal. Like `mismatched-closing-tag`, it's reported during validation, with or without a component model, and the file still gets its other diagnostics.

```text
error[number-literal-out-of-range]: this number is too large: the largest number MPRX can represent is about 1.8e308
```

**Fix:** use a smaller number. MPRX numbers are binary64, as in JavaScript.

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

```text
error[manifest-syntax-error]: the manifest isn't valid JSON: trailing comma
 --> examples/fixtures/manifest/fail/syntax-error.json:6:3
  |
6 |   }
  |   ^
```

Only the first JSON syntax error is reported. A leading byte-order mark is allowed.

**Fix:** correct the JSON. Trailing commas and comments aren't allowed.

### `manifest-nesting-too-deep`

The manifest is valid JSON, but its arrays and objects nest more than 128 levels deep, the deepest MESH supports. The manifest format sets no limit: this one is MESH's, and a later version may raise it. The document itself is level 1. This is checked right after the JSON syntax, before anything else, and reported at the first `[` or `{` too deep.

```text
error[manifest-nesting-too-deep]: the manifest nests arrays and objects more than 128 levels deep, the deepest MESH supports
   --> examples/fixtures/manifest/fail/nesting-too-deep.json:131:1
    |
131 | { "kind": "string" }
    | ^
```

**Fix:** declare deeply nested types as named types (`"kind": "named"`) instead of writing them out in place.

### `manifest-unsupported-version`

The manifest's `"version"` is missing, or isn't `1`, the only version this MESH reads. Another version has another format, so nothing else in the manifest is checked.

```text
error[manifest-unsupported-version]: unsupported manifest version 2; this MESH reads version 1
 --> examples/fixtures/manifest/fail/unsupported-version.json:2:14
  |
2 |   "version": 2,
  |              ^
```

**Fix:** write `"version": 1`, and the rest of the manifest in version 1's format.

### `manifest-invalid-value`

A value has the wrong JSON type: an array where an object belongs, a string for `"required"`, a type that isn't an object, and so on.

```text
error[manifest-invalid-value]: "required" must be true or false, found a string
  --> examples/fixtures/manifest/fail/invalid-value.json:15:65
   |
15 |         "compact": { "type": { "kind": "boolean" }, "required": "no" }
   |                                                                 ^^^^
```

**Fix:** use the JSON type the message names.

### `manifest-missing-property`

An object is missing a property it must have. Points at the object's opening `{`.

```text
error[manifest-missing-property]: prop "size" is missing the property "required"
  --> examples/fixtures/manifest/fail/missing-property.json:15:17
   |
15 |         "size": { "type": { "kind": "string" } }
   |                 ^
```

- The manifest needs `"version"`, `"types"` and `"components"`.
- A component needs all of `"props"`, `"events"`, `"commands"` and `"scope"`, even when they're empty.
- A prop or record field needs both `"type"` and `"required"`. `"required"` has no default.
- A type needs a `"kind"`, plus `"element"` for a list, `"fields"` for a record, `"name"` for a named type, and `"type"` for an optional type.

**Fix:** add the property.

### `manifest-unknown-property`

An object has a property its format doesn't allow. Points at the property's key.

```text
error[manifest-unknown-property]: prop "size" can't have the property "placeholder"
  --> examples/fixtures/manifest/fail/unknown-property.json:15:68
   |
15 |         "size": { "type": { "kind": "string" }, "required": false, "placeholder": "md" }
   |                                                                    ^^^^^^^^^^^^^
```

**Fix:** remove the property. A type has only the properties of its kind, and a prop has only `type`, `required` and `default` (a record field has `type` and `required`).

### `manifest-duplicate-key`

A key appears twice in one object. Most JSON tools silently keep the last one; MESH reports the repeat instead, and uses the first occurrence. Points at the second occurrence.

```text
error[manifest-duplicate-key]: component "template" declares the prop "size" twice
  --> examples/fixtures/manifest/fail/duplicate-key.json:16:9
   |
16 |         "size": { "type": { "kind": "number" }, "required": false }
   |         ^^^^^^
```

This covers every level: component names, props, events, commands, scope names, named types, record fields, and properties such as `"type"`.

**Fix:** delete or rename one of them.

### `manifest-unknown-kind`

A type's `"kind"` isn't one of `string`, `number`, `boolean`, `null`, `any`, `list`, `record`, `named`, `optional`, `enum` or `snippet`.

```text
error[manifest-unknown-kind]: unknown type kind "integer"; expected string, number, boolean, null, any, list, record, named, optional, enum or snippet
  --> examples/fixtures/manifest/fail/unknown-kind.json:18:28
   |
18 |         "count": { "kind": "integer" }
   |                            ^^^^^^^^^
```

`void` and `nothing` are types MESH uses internally, and a manifest can't write them either.

**Fix:** use one of the kinds above. For a number, use `number`: there's no separate integer type.

`enum` was added in 0.14. An older MESH reading a manifest that uses it reports this code with the older list of kinds; upgrade MESH, or type the value `string`.

### `manifest-invalid-name`

A declared name can't be written in MPRX where it would be used.

```text
error[manifest-invalid-name]: prop name "aria-label" isn't a valid MPRX identifier: start with a letter or `_`, then use letters, digits or `_`
  --> examples/fixtures/manifest/fail/invalid-name.json:15:9
   |
15 |         "aria-label": { "type": { "kind": "string" }, "required": false }
   |         ^^^^^^^^^^^^
```

- A component name must be a valid tag name: a letter or `_`, then letters, digits, `_` or `-`.
- Every other name (props, events, commands, command parameters, scope names, named types, record fields) must be a valid identifier: a letter or `_`, then letters, digits or `_`. No `-`, because MPRX reads `-` as minus.
- None of those identifiers can be `true`, `false` or `null`. MPRX's lexer always reads those words as literals, so a prop, scope name or field spelled that way could never be referred to.

**Fix:** rename it, for example `aria-label` to `ariaLabel`.

### `manifest-duplicate-parameter`

A command declares two parameters with the same name. Points at the second one's name.

```text
error[manifest-duplicate-parameter]: command "move" has two parameters named "item"
  --> examples/fixtures/manifest/fail/duplicate-parameter.json:20:23
   |
20 |             { "name": "item", "type": { "kind": "number" } }
   |                       ^^^^^^
```

**Fix:** rename one of them.

### `manifest-unknown-type`

A `named` type refers to a name that `"types"` doesn't declare. Points at the name.

```text
error[manifest-unknown-type]: unknown type "Usr": the manifest's "types" doesn't declare it
  --> examples/fixtures/manifest/fail/unknown-type.json:25:44
   |
25 |         "user": { "kind": "named", "name": "Usr" }
   |                                            ^^^^^
```

**Fix:** declare the type in `"types"`, or correct the name. Names are case-sensitive.

### `manifest-recursive-type`

A named type refers to itself, directly or through other named types. Each cycle is reported once, at the first of its types in the manifest.

```text
error[manifest-recursive-type]: type "Folder" refers to itself: Folder -> Folders -> Folder
 --> examples/fixtures/manifest/fail/recursive-type.json:4:5
  |
4 |     "Folder": {
  |     ^^^^^^^^
```

Recursive types aren't supported in version 1.

**Fix:** break the cycle, for example by typing the recursive part `any`.

### `manifest-nested-optional`

An `optional` type wraps a type that is already optional: another `optional`, or a named type that is one. Points at the inner type.

```text
error[manifest-nested-optional]: an optional type can't wrap "MaybeName", which is already optional
  --> examples/fixtures/manifest/fail/nested-optional.json:20:74
   |
20 |         "name": { "kind": "optional", "type": { "kind": "named", "name": "MaybeName" } }
   |                                                                          ^^^^^^^^^^^
```

**Fix:** remove one of the two. "Optional optional" means the same as "optional".

### `manifest-enum-too-large`

An `enum` type lists more than 256 values. Points at the `"values"` array.

```text
error[manifest-enum-too-large]: an enum has 257 values, more than the 256 MESH supports; keep a larger set as `string` and validate it in the application
  --> examples/fixtures/manifest/fail/enum-too-large.json:10:23
   |
10 |             "values": [
   |                       ^
```

The limit keeps a variant set small enough to read in a diagnostic and to check at every host boundary.

**Fix:** keep a larger set as `string` and validate it in the application, or split it into several smaller enums.

### `manifest-duplicate-variant`

An `enum` lists the same value twice. Points at the second occurrence. An enum is a set; the order is only used to word messages.

```text
error[manifest-duplicate-variant]: the enum lists the value "a" twice
 --> examples/fixtures/manifest/fail/duplicate-variant.json:7:66
  |
7 |         "tone": { "type": { "kind": "enum", "values": ["a", "b", "a"] }, "required": true }
  |                                                                  ^^^
```

**Fix:** delete the repeat.

### `manifest-snippet-misplaced`

A `snippet` type is somewhere but a component's prop or scope name (or an `optional` of one there). A snippet is never stored, returned or sent, so it isn't a record's field, a list's element, a named type, an event's payload, a command's parameter, or another snippet's parameter. Points at the `"snippet"`.

```text
error[manifest-snippet-misplaced]: a snippet type can only be the type of a component's prop or scope name: it is never stored, returned or sent, so it is not a record field, a list element, a named type, an event payload or a command parameter, and not a snippet's own parameter
 --> examples/fixtures/manifest/fail/snippet-misplaced.json:8:79
  |
8 |           "type": { "kind": "record", "fields": { "body": { "type": { "kind": "snippet", "params": [] }, "required": true } } },
  |                                                                               ^^^^^^^^^
```

**Fix:** pass the data a snippet would need instead, and give the snippet to the component through a prop.

### `manifest-children-not-snippet`

A prop named `children` isn't a `snippet` with no parameters. `children` is reserved for the content written between a component's tags, which a component with a template receives as a snippet. Points at the prop's name.

```text
error[manifest-children-not-snippet]: the prop `children` is the content written inside the component, so its type must be a snippet with no parameters: { "kind": "snippet" }
 --> examples/fixtures/manifest/fail/children-not-snippet.json:7:9
  |
7 |         "children": { "type": { "kind": "string" }, "required": true }
  |         ^^^^^^^^^^
```

**Fix:** type it `{ "kind": "snippet" }`, or give the prop another name.

### `manifest-missing-component`

The manifest loaded, but it doesn't declare the component the file is the template of. That component is the file's name without its extension (`users-page.mprx` is `users-page`), unless `--component` names another. Points at the manifest's `"components"` key.

```text
error[manifest-missing-component]: the manifest declares no component "template", which this file is the template of
 --> examples/fixtures/manifest/fail/missing-component.json:4:3
  |
4 |   "components": {
  |   ^^^^^^^^^^^^
```

**Fix:** declare the component, or pass `--component <NAME>`.

---

## Model errors

With `--model`, a file that is valid MPRX is also checked against the manifest, as the template of one component (the file's name without its extension, or `--component`):

- every element is an instance of a component the manifest declares, and its attributes and `on.` bindings are that component's props and events;
- every reference is a name in the template's `scope`, and every command is one of the template's `commands`;
- commands and `$event` appear only where they may;
- every expression has a type, and the operators and conditionals in it are used with the types they need;
- every prop value, command argument and object literal field has a type that fits its declaration.

Each mistake is reported once, at the name or expression that's wrong. When a declared name is close to the one written, a `help` line suggests it. A file with a syntax error isn't checked against the model.

### `unknown-component`

An element's tag names a component the manifest doesn't declare. MESH has no built-in elements: `page`, `text` and `button` are components like any other, and the manifest must declare them. Points at the tag name.

```text
error[unknown-component]: unknown component "avatr": the manifest doesn't declare it
 --> examples/fixtures/check/fail/unknown-component.mprx:2:4
  |
2 |   <avatr src={user.avatar} />
  |    ^^^^^
  = help: did you mean "avatar"?
```

The element's attributes and `on.` bindings aren't checked, since there's no component to check them against. The expressions in them still are.

**Fix:** declare the component in the manifest, or correct the tag. Names are case-sensitive.

### `unknown-prop`

An attribute isn't one of the props the element's component declares. Points at the attribute name.

```text
error[unknown-prop]: component "avatar" has no prop "sise"
 --> examples/fixtures/check/fail/unknown-prop.mprx:1:27
  |
1 | <avatar src={user.avatar} sise="sm" />
  |                           ^^^^
  = help: did you mean "size"?
```

**Fix:** correct the name, or declare the prop in the manifest.

### `missing-required-prop`

An element doesn't supply a prop its component declares with `"required": true`. Points at the tag name; each missing prop is reported separately.

```text
error[missing-required-prop]: component "avatar" requires the prop "src"
 --> examples/fixtures/check/fail/missing-required-prop.mprx:1:2
  |
1 | <avatar size="sm" />
  |  ^^^^^^
```

A required prop must be written even when its *type* is optional: `"src": { "type": { "kind": "optional", ... }, "required": true }` means the value may be absent, not that the attribute may be left out. The two are independent.

**Fix:** supply the prop, or declare it `"required": false`.

### `unknown-event`

An `on.<name>` binding names an event the element's component doesn't declare. Points at the event name.

```text
error[unknown-event]: component "button" has no event "clik"
 --> examples/fixtures/check/fail/unknown-event.mprx:1:12
  |
1 | <button on.clik={save()}>Save</button>
  |            ^^^^
  = help: did you mean "click"?
```

**Fix:** correct the name, or declare the event in the manifest.

### `unknown-reference`

An expression refers to a name that isn't in the template's `scope`. Points at the name.

```text
error[unknown-reference]: unknown reference "usr": it isn't in the template's scope
 --> examples/fixtures/check/fail/unknown-reference.mprx:1:14
  |
1 | <page title={usr.name} />
  |              ^^^
  = help: did you mean "user"?
```

The scope is everything an expression can refer to. Props aren't in it automatically: if the template needs a prop's value, the manifest lists that name in `scope` too. A reference that isn't in scope isn't checked any further, so `usr.name` gets this one error and nothing about `.name`.

**Fix:** correct the name, or add it to the template component's `scope`.

### `unknown-command`

A handler invokes a command that the template's component doesn't declare. Points at the command name.

```text
error[unknown-command]: unknown command "sav": the template's component doesn't declare it
 --> examples/fixtures/check/fail/unknown-command.mprx:1:19
  |
1 | <button on.click={sav()}>Save</button>
  |                   ^^^
  = help: did you mean "save"?
```

**Fix:** correct the name, or declare the command in the template component's `commands`.

### `command-arity-mismatch`

A command is invoked with more or fewer arguments than it declares parameters. Every parameter is required. Points at the whole invocation. The same holds for a handler that forwards one of the component's own events ([Composite events](templates.md#composite-events)): an event with a payload takes exactly one argument, and one without takes none.

```text
error[command-arity-mismatch]: command "select" takes 1 argument, but 0 were given
 --> examples/fixtures/check/fail/command-arity-mismatch.mprx:1:19
  |
1 | <button on.click={select()}>Select</button>
  |                   ^^^^^^^^
```

**Fix:** pass exactly one argument per parameter, in order.

### `command-outside-handler`

A command is invoked anywhere but as the whole handler of an `on.` binding: in an attribute value, in content, inside an operator, or as another command's argument. A command is an action, not a value. Points at the invocation, and nothing inside it is checked.

```text
error[command-outside-handler]: command "save" can only be invoked as the whole handler of an `on.` binding
 --> examples/fixtures/check/fail/command-outside-handler.mprx:1:14
  |
1 | <page title={save()} />
  |              ^^^^^^
```

**Fix:** move the command into an `on.` binding, such as `on.click={save()}`.

### `handler-not-command`

An `on.` binding's handler isn't a command invocation. Points at the handler, and nothing inside it is checked.

```text
error[handler-not-command]: the handler of `on.click` must be a command invocation, like `save()`
 --> examples/fixtures/check/fail/handler-not-command.mprx:1:19
  |
1 | <button on.click={user.name}>Select</button>
  |                   ^^^^^^^^^
```

**Fix:** invoke a command: `on.click={select(user)}`.

### `event-value-outside-handler`

`$event` is used outside the arguments of an `on.` handler's command. It's the value the event carries, so it only exists there.

```text
error[event-value-outside-handler]: `$event` can only be used in the arguments of an `on.` handler's command
 --> examples/fixtures/check/fail/event-value-outside-handler.mprx:1:14
  |
1 | <page title={$event} />
  |              ^^^^^^
```

`$event` can appear anywhere inside a handler's arguments, including inside an object or array: `on.change={update({ name: $event })}`.

**Fix:** use `$event` only as (part of) an argument of the command in an `on.` handler.

### `event-has-no-payload`

`$event` is used in the handler of an event that the manifest declares without a `"payload"`, so there is no value.

```text
error[event-has-no-payload]: event "press" of component "button" carries no value, so there is no `$event`
 --> examples/fixtures/check/fail/event-has-no-payload.mprx:1:26
  |
1 | <button on.press={select($event)}>Select</button>
  |                          ^^^^^^
```

**Fix:** don't use `$event` in this handler, or declare the event's `"payload"` type in the manifest.

### `unknown-special-value`

A `$` name other than `$event`. `$event` is the only special value.

```text
error[unknown-special-value]: unknown special value `$evnt`: the only one is `$event`
 --> examples/fixtures/check/fail/unknown-special-value.mprx:1:26
  |
1 | <button on.click={select($evnt)}>Select</button>
  |                          ^^^^^
  = help: did you mean "$event"?
```

**Fix:** write `$event`.

### `unknown-member`

A member access reads a field the value doesn't have: the record type has no such field, or the value isn't a record at all. Strings, numbers and lists have no members. Points at the member name.

```text
error[unknown-member]: User has no member "nmae"
 --> examples/fixtures/check/fail/unknown-member.mprx:1:13
  |
1 | <text>{user.nmae}</text>
  |             ^^^^
  = help: did you mean "name"?
```

A value of type `any` has every member, and reading one gives `any`.

**Fix:** correct the name, or declare the field in the record type.

### `possibly-absent-access`

A member access reads a field of a value that may be absent: its type is optional (`T?`), `any?` included. MPRX has no optional chaining; test the value with `has(x)` or give it a default with `x ?? y` instead. Points at the value.

```text
error[possibly-absent-access]: this value may be absent (its type is User?), so its member "name" can't be read; MPRX has no optional chaining
 --> examples/fixtures/check/fail/possibly-absent-access.mprx:1:8
  |
1 | <text>{maybeUser.name}</text>
  |        ^^^^^^^^^
```

Reading a field that is itself optional is fine: `user.avatar`, where `avatar` is declared `"required": false`, has type `string?`. It's reading *through* it, as in `user.avatar.length`, that fails.

**Fix:** give the template a value that is always present, for example a scope name typed `User` rather than `User?`, or give the missing case a value: `(maybeUser ?? fallbackUser).name`.

### `type-mismatch`

A value's type doesn't fit where it's used. Points at the value, and the message says what was needed and what was found.

```text
error[type-mismatch]: an operand of `+` needs number, found string
 --> examples/fixtures/check/fail/type-mismatch.mprx:1:16
  |
1 | <text>{count + name}</text>
  |                ^^^^
```

What each place needs:

| Where | Needs |
|---|---|
| `!x`, `x && y`, `x \|\| y`, the condition of `c ? a : b` | `boolean` |
| `-x`, `x + y`, `x - y`, `x * y`, `x / y`, `x % y`, `x < y`, `x <= y`, `x > y`, `x >= y` | `number` |
| a prop's value | the prop's declared type |
| a command's argument, `$event` included | the parameter's declared type |
| a field's value in an object literal given to a record | the field's declared type |
| an element of an array literal given to a list | the list's element type |
| each branch of `c ? a : b` given a type | that type |

Nothing is converted to fit: `"yes"` is a `string`, not a `boolean`. A value written as a quoted attribute, like `size="lg"`, is a `string` too.

```text
error[type-mismatch]: the prop "disabled" of component "button" needs boolean, found string
 --> examples/fixtures/check/fail/not-assignable-rule-6-primitives.mprx:1:19
  |
1 | <button disabled={"yes"}>Save</button>
  |                   ^^^^^
```

A prop declared optional (`"required": false`) may be left out, but a value you do give it must fit its type: an optional *declaration* doesn't make the *value* optional.

`+` adds numbers only: there's no string concatenation. There are no implicit conversions either, so `"1"` isn't a number and `0` isn't `false`.

A value of type `any` fits everywhere a present value does. A value that may be absent (`T?`, including `any?`) fits only where absence is allowed. When absence is the only reason it doesn't fit, the message ends with `which may be absent`:

```text
error[type-mismatch]: the condition needs boolean, found boolean?, which may be absent
 --> examples/fixtures/check/fail/condition-may-be-absent.mprx:1:8
  |
1 | <text>{maybeFlag ? "on" : "off"}</text>
  |        ^^^^^^^^^
```

**Fix:** use a value of the needed type.

### `no-common-type`

Two types that must have a common type don't: the operands of `==` or `!=`, the two branches of `c ? a : b`, the elements of an array, the two sides of `x ?? y`, or what two renders give one parameter of a local snippet. Points at the comparison or conditional, or at the first array element that doesn't fit with the ones before it.

```text
error[no-common-type]: `==` compares string with number, which have no common type
 --> examples/fixtures/check/fail/no-common-type.mprx:1:8
  |
1 | <text>{name == 1}</text>
  |        ^^^^^^^^^
```

Where a type is expected, such as a prop's value, an array literal is checked against it element by element and a conditional branch by branch, so they need no common type there: `[{ name: "Ada", active: true }, { name: "Bo", active: false, avatar: "bo.png" }]` fits a `list<User>` prop, and a wrong element or branch is a `type-mismatch` at itself.

Equality is strict, with no conversions: a string never equals a number, so comparing them is a mistake rather than `false`. Two types have a common type when they are the same, when one is `any`, or when one is only optional where the other isn't: `string` and `string?` have the common type `string?`. `null` is a value, not absence, so `maybeName == null` compares `string?` with `null` and has no common type. An empty array `[]` fits with any list. Two enums have the union of their values as the common type, and an enum and `string` have `string`.

**Fix:** compare or combine values of the same type.

### `duplicate-object-key`

An object literal has the same key more than once. As with a duplicate attribute, the last occurrence is the one that counts: each earlier one is reported, at its key.

```text
error[duplicate-object-key]: duplicate object key "first": this occurrence is shadowed by a later one
 --> examples/fixtures/check/fail/duplicate-object-key.mprx:1:20
  |
1 | <probe anything={{ first: name, first: "Ada" }} />
  |                    ^^^^^
```

The shadowed value is still checked for its own mistakes, but it isn't part of the object, so it isn't checked against a record's field.

**Fix:** remove or rename one of them.

### `unknown-field`

An object literal, given where a record type is expected, has a key the record doesn't declare. Records are exact: a field the component model doesn't declare is never silently accepted. Points at the key.

```text
error[unknown-field]: User has no field "nickname"
 --> examples/fixtures/check/fail/unknown-field.mprx:1:43
  |
1 | <probe user={{ name: "Ada", active: true, nickname: "A" }} />
  |                                           ^^^^^^^^
```

**Fix:** remove the key, or correct its name.

### `missing-required-field`

An object literal, given where a record type is expected, lacks a field the record declares with `"required": true`. Points at the object literal; each missing field is reported separately.

```text
error[missing-required-field]: the object is missing the field "active", which User requires
 --> examples/fixtures/check/fail/missing-required-field.mprx:1:14
  |
1 | <probe user={{ name: "Ada" }} />
  |              ^^^^^^^^^^^^^^^
```

As with props, a required field must be written even when its type is optional.

**Fix:** add the field.

### `unknown-variant`

A string literal is written where an `enum` is expected, or is compared with one, but it isn't one of the enum's values. The sites are: an attribute value, `==` and `!=`, the branches of `?:`, and the elements of an array literal. Points at the literal. When a value is close in spelling, the help line suggests it. A list of values longer than eight is cut after eight.

```text
error[unknown-variant]: "dagner" is not one of "info", "warning", "danger"
 --> examples/fixtures/check/fail/unknown-variant.mprx:1:13
  |
1 | <badge tone="dagner"></badge>
  |             ^^^^^^^^
  = help: did you mean "danger"?
```

Only a literal is checked this way. A string-typed expression doesn't fit an enum (that is a `type-mismatch`), and there is no narrowing: `if x == "info"` does not make `x` an enum.

**Fix:** write one of the listed values, or add the value to the enum in the manifest.

### `redundant-presence-test`

*Warning.* `has(x)` tests a value that is never absent, so it is always `true`. Points at the `has(x)`.

```text
warning[redundant-presence-test]: this value is string, which is never absent, so `has` is always true
 --> examples/fixtures/check/warn/redundant-presence-test.mprx:1:8
  |
1 | <text>{has(name)}</text>
  |        ^^^^^^^^^
```

`has(x)` is `true` when `x` is present and `false` when it is absent. `null` is a value, so `has(null)` is `true`. Calling `has` with other than one argument is a `syntax-error`. `has` is not reserved: a handler's `has(...)` is still a command call.

**Fix:** delete the test, or test a value whose type is optional (`T?`).

### `redundant-default`

*Warning.* `x ?? y` has a left side that is never absent, so `y` is never used. Points at the whole expression.

```text
warning[redundant-default]: the left side is string, which is never absent, so the default is never used
 --> examples/fixtures/check/warn/redundant-default.mprx:1:8
  |
1 | <text>{name ?? "anonymous"}</text>
  |        ^^^^^^^^^^^^^^^^^^^
```

`x ?? y` is `x` when `x` is present, otherwise `y`. `y` is evaluated only when it is needed. `y` must be present: if it may be absent that is the existing `type-mismatch` ("which may be absent"), so the result of `??` is never absent. If the two have no common type, that is `no-common-type`.

**Fix:** delete the `?? y`, or apply it to an optional value.

### `unknown-snippet`

`{@render name(…)}` names no snippet: none is declared among the children it is in or around, and the template's scope has no snippet-typed name of that name. Points at the name, with a did-you-mean when one is close.

```text
error[unknown-snippet]: unknown snippet "nothing": it is neither declared among the children it is in or around, nor a snippet of this component
 --> examples/fixtures/check/fail/unknown-snippet.mprx:1:26
  |
1 | <page title="x">{@render nothing()}</page>
  |                          ^^^^^^^
```

**Fix:** declare it with `{#snippet name(parameters)} … {/snippet}` in the same children or around, or declare a snippet prop of that name in the manifest.

### `snippet-arity-mismatch`

A render gives a snippet a different number of arguments than it has parameters, or a snippet passed to a component is written with a different number of parameters than the prop's type has. A render points at the whole tag, a declaration at its name.

```text
error[snippet-arity-mismatch]: the snippet "pair" takes 2 parameters, found 1
 --> examples/fixtures/check/fail/snippet-arity-mismatch.mprx:3:3
  |
3 |   {@render pair(name)}
  |   ^^^^^^^^^^^^^^^^^^^^

error[snippet-arity-mismatch]: the snippet "row" takes 1 parameter, found 2
 --> examples/fixtures/check/fail/snippet-arity-mismatch.mprx:4:38
  |
4 |   <data-list items={users}>{#snippet row(u, v)}<text>{u.name}</text>{/snippet}</data-list>
  |                                      ^^^
```

**Fix:** give exactly as many arguments as parameters.

### `duplicate-snippet`

Two snippets have one name among the same children. Points at the second's name.

```text
error[duplicate-snippet]: a second snippet named "one" among the same children
 --> examples/fixtures/check/fail/duplicate-snippet.mprx:3:13
  |
3 |   {#snippet one()}<text>b</text>{/snippet}
  |             ^^^
```

**Fix:** rename one.

### `children-conflict`

A component that takes `children` is given them twice: as an attribute `children={…}` and as the content between its tags. Points at the attribute.

```text
error[children-conflict]: "panel" gets its `children` twice: as an attribute and as the content between its tags; write one
 --> examples/fixtures/check/fail/children-conflict.mprx:3:20
  |
3 |   <panel title="t" children={s}><text>b</text></panel>
  |                    ^^^^^^^^
```

**Fix:** write one of them.

### `content-not-text`

An interpolation in content, `<text>{...}</text>`, whose type is a list or a record (or an optional of one) has no text form, so it can't be content (spec §9.7.8). Points at the interpolated expression.

```text
error[content-not-text]: a value of type list<User> can't be shown as text: interpolate one of its fields, or a string, number, boolean or null
 --> examples/fixtures/check/fail/content-not-text.mprx:1:8
  |
1 | <text>{users}</text>
  |        ^^^^^
```

A value of type `any` isn't checked here: if it turns out to be a list or a record at runtime, the runtime reports it instead.

**Fix:** interpolate a field (`{user.name}`), or pass the value to a component that shows it as a prop.

---

### `retired-element`

A reserved element that [blocks](templates.md#blocks) replaced, written as an element: `<mesh-if>`, `<mesh-each>`, `<mesh-switch>`, `<mesh-case>` or `<mesh-default>`. Control flow is syntax, not an element, so the message names the block to write instead: `{#if condition} … {/if}`, `{#each items as item (key)} … {/each}`, or a chain `{#if a} … {:else if b} … {:else} … {/if}`. Points at the tag name. The element is reported once: not also as an unknown component.

```text
error[retired-element]: `<mesh-if>` is no longer an element: write a block: `{#if condition} … {/if}`
 --> fixtures/check/fail/retired-element.mprx:1:18
  |
1 | <page title="x"><mesh-if when={flag}><text>x</text></mesh-if></page>
  |                  ^^^^^^^
```

**Fix:** write the block the message names. `<mesh-if when={a}>X</mesh-if>` is `{#if a}X{/if}`; `<mesh-each items={xs} as="x" key={x.id}>X</mesh-each>` is `{#each xs as x (x.id)}X{/each}`; a `mesh-switch` of cases is `{#if a}…{:else if b}…{:else}…{/if}`.

---

### `missing-repeat-key`

An `{#each}` block without its `(key)`. The key identifies each item across updates, so it is mandatory: write `{#each items as item (key)}`, with a string or a number that is different for each item. Points at the `{#each` keyword. A key that is present but has the wrong type is a `type-mismatch` ("the key of `{#each}`").

```text
error[missing-repeat-key]: `{#each}` needs a key, which identifies each item across updates: write `{#each items as item (key)}`, with a string or a number that is different for each item
 --> fixtures/check/fail/missing-repeat-key.mprx:2:3
  |
2 |   {#each users as person}
  |   ^^^^^^
```

**Fix:** add the key after the item's name: `{#each users as person (person.id)}`.

---

### `invalid-switch`

**Retired.** Nothing reports this code any more: the multi-way element it described (`mesh-switch` with `mesh-case` and `mesh-default`) was replaced by a chain of `{:else if}` branches in an `{#if}` block, which cannot be malformed in those ways, and writing the element at all is a [`retired-element`](#retired-element). The code stays in the catalogue because a code is never reused for a different meaning.

---

## Assembly errors

The runtime reports these before it evaluates anything, when it validates a program (`docs/manual/templates.md`, "The assembly rules"), and so does the compiler's program check, with the same codes and locations. Each is in a runtime diagnostics document.

### `assembly-malformed-template`

A template isn't valid against `template-v1` or `template-v2` (a version 1 document that holds `has` or `??` is malformed: they are version 2), or names something the model doesn't declare with the kind the template gives it. Also reported for a template that isn't JSON, or whose `format` isn't `mesh-template`.

Location: `template`.

### `assembly-unsupported-format-version`

A template's `version` is a number this runtime doesn't support. This runtime reads versions 1 and 2; a runtime from before 0.14 reads only version 1, so it refuses a template that uses `has` or `??` (the compiler writes version 2 for those, and version 1 for everything else).

**Fix:** recompile with a MESH whose version the runtime reads, or update the runtime.

Location: `template`.

### `assembly-fingerprint-mismatch`

A template's fingerprint isn't the model's: it was checked against another model. Recompile it.

Location: `template`.

### `assembly-duplicate-template`

The program has more than one template for a component.

Location: `source`, the root element of each template of that component after the first.

### `assembly-missing-root`

The program has no template for its root component.

Location: `program`.

### `assembly-unbound-scope-name`

A composite used as a composite occurrence has a scope name that no prop of the same name binds.

Location: `source`, each such composite occurrence.

### `assembly-unsound-binding`

A prop binds a composite's scope name unsoundly: its type isn't assignable to the scope name's, or it's optional and the scope name isn't.

Location: `source`, each such composite occurrence.

### `assembly-cycle`

A composite expands itself, directly or through others.

Location: `source`, every composite occurrence on the cycle.

### `assembly-composite-event`

A composite declares an event and a command of the same name. A composite may declare events (its template forwards them, [Composite events](templates.md#composite-events)), but then a handler in its template that names one of them would be either, so the names must differ.

Location: `source`, every composite occurrence of it.

### `assembly-composite-children`

A composite occurrence has children that the composite's template has no slot for. A composite takes loose children (text or elements outside a `mesh-fill`) exactly when its template has a default `mesh-slot` ([Children and the slot](templates.md#children-and-the-slot)); and a `mesh-fill` needs a `mesh-slot` of its name, once, in an occurrence of a composite ([Named slots](templates.md#named-slots)). An occurrence of a composite that has slots may have no children.

Location: `source`, the occurrence.

### `assembly-snippet-to-primitive`

A snippet was given to a component that has no template in the program, so it is a primitive, and a primitive's props go to the renderer, which cannot take a snippet. Reported where a primitive's prop of snippet type is written, a snippet is declared among its children for a prop of that name, or content is written inside a primitive that declares a `children` snippet prop. Nothing is rendered.

Location: `source`, the occurrence.

**Fix:** give the component a template, or don't pass it a snippet.

### `assembly-snippet-root-scope`

The root component's scope has a snippet-typed name. The root's scope comes from the host's snapshot, which holds no snippet.

Location: `program`.

**Fix:** make the root take plain data, and put the snippet in a composite it uses.

### `assembly-root-fragment`

The program's root renders one node, but its root template's root is a `mesh-fragment`, or a composite whose template starts with one ([Fragments](templates.md#fragments)), among them a template that is only a `{@render}` ([a forwarder](templates.md#snippets)). A fragment makes no node. Points at the root of the root template.

Location: `source`, the root template's root.

**Fix:** put an element around it, or give the root a template that starts with an element.

## Runtime errors

The runtime reports these while rendering or dispatching (`docs/manual/runtime.md`): input errors about the host's values, evaluation errors from §9.7's checks, and one internal error. Every runtime diagnostic is an error, in a runtime diagnostics document (`schemas/runtime-diagnostics-v1.schema.json`), and its `location` has the form its entry names.

### Input errors

These are reported by input validation.

### `runtime-missing-value`

A value that must be present is absent: a scope name whose type isn't optional, a record field that doesn't read as optional, or a payload for an event that has one and a type that isn't optional.

Location: `input`, the path of the missing value; a missing payload is `["$event"]`.

### `runtime-value-mismatch`

A present value doesn't fit its type (§9.7.2). It is reported at the deepest path where it stops fitting: a list's element, a record's field. A string that isn't one of an `enum`'s values doesn't fit it, so it is reported here too, wherever it crosses the host boundary: a snapshot field, an event payload, a value read through `any`; the message lists the enum's values. The same check reports `runtime-prop-mismatch` for a prop and `runtime-argument-mismatch` for a command argument.

Location: `input`, the deepest path where the value stops fitting.

### `runtime-unknown-field`

A record has a field its type doesn't declare. Records are exact (§9.8.4).

Location: `input`, the field's path.

### `runtime-absent-element`

A list holds an absent element: JavaScript's `undefined`, or a hole. Location: the element's path.

Location: `input`, the element's path.

### `runtime-number-out-of-range`

A JSON number too large for binary64: its nearest value would be infinite.

Location: `input`, the number's path.

### `runtime-non-finite-input`

A number that is NaN or an infinity, which JavaScript can express and JSON can't.

Location: `input`, the number's path.

### `runtime-unpaired-surrogate`

A string, or a record field's name, that isn't a sequence of Unicode scalar values: it has an unpaired UTF-16 surrogate.

Location: `input`, the string's path, or, for a field name, the field's.

### `runtime-unsupported-value`

A JavaScript value outside the boundary data model: a function, a symbol, a `bigint`, an object that isn't a plain object or an array, or a cycle. The message names its kind (§9.8.6).

Location: `input`, the value's path.

### `runtime-unexpected-payload`

A payload was given for an event declared without one.

Location: `input`, `["$event"]`.

### `runtime-handler-other-program`

Dispatch was given a handler identifier made by another program, or one that isn't a handler identifier at all.

Location: `handler`.

### `runtime-unknown-handler`

Dispatch was given a handler identifier of the render's program that names no handler in it or, in a program with a conditional, whose node isn't in the render (spec §9.10.7).

Location: `handler`.

### Evaluation errors

These are reported by evaluation, one per call, located at the span of the value checked (§9.7.6). A clean check guarantees none of the type checks can fail, except where a value has type `any` or `any?`.

### `runtime-operand-mismatch`

An operand isn't of the kind its operator needs: a boolean for `!`, `&&`, `||` or a conditional's condition; a number for unary `-`, arithmetic or a comparison.

Location: `source`.

### `runtime-not-a-record`

Member access on a value that isn't a record.

Location: `source`.

### `runtime-missing-member`

Member access, on a value of type `any`, to a field that's absent.

Location: `source`.

### `runtime-content-not-text`

An interpolation of type `any` or `any?` whose value is a list or a record (§9.7.8).

Location: `source`.

### `runtime-prop-mismatch`

A prop's value, at a primitive or composite occurrence, doesn't fit the prop's declared type, or is absent where the type isn't optional.

Location: `source`.

### `runtime-argument-mismatch`

A command argument doesn't fit its parameter's type.

Location: `source`.

### `runtime-non-finite-output`

NaN or an infinity would reach an output: a primitive prop, a text run or a command argument, at any depth.

Location: `source`.

### `runtime-absent-element-output`

A list holding an absent element would reach an output, at any depth.

Location: `source`.

### Repeated structure (provisional)

These two belong to the provisional `mesh-each` ([runtime manual](runtime.md#repeated-structure-provisional)); their names and the construct are not final.

### `runtime-invalid-key`

An item of a repeat has no usable declared key: the `key` expression is absent for it, or is neither a string nor a finite number (`null`, a boolean, a list, a record, `NaN` and the infinities are not keys). Nothing is rendered; a key is never invented from the item's position.

Location: `source`, the `key` expression.

### `runtime-duplicate-key`

Two items of one repeat declare the same key, under strict equality: the same kind (`"1"` is not `1`) and the same value (`0` and `-0` are one). Nothing is rendered; the author's key was meant to be unique, and the runtime does not pick one of the items.

Location: `source`, the `key` expression of the later item.

### Changes

These belong to `update` from changes ([runtime manual](runtime.md#changes)). A value that an edit gives and that doesn't fit its type is reported by the input codes above (`runtime-value-mismatch` and the rest), at its path in the snapshot.

### `runtime-changes-base-mismatch`

The changes were computed against another render than the one given: their `base` is not the render's version. Nothing is applied. This is the check that the host and the runtime agree on which snapshot the changes edit; changes for a render that has since been updated, or for a different one, are refused rather than applied to a snapshot they weren't made for.

Location: `input`, `["base"]`.

### `runtime-invalid-change`

The changes document, or one edit, can't be applied: the document isn't `{ base, changes }`; an edit isn't `{ op, path, value? }`; `op` isn't `set`, `insert` or `remove`; a `path` is empty or has a segment that is neither a field name nor a non-negative integer; a `value` is missing from a `set` or `insert`, or given to a `remove`; or a path leads nowhere the type allows (a field a record doesn't have, an index past the end of a list, a record indexed like a list, a scope name that isn't one, removing a field the type doesn't let be absent). Nothing is applied: the edits are all or none.

Location: `input`, the path in the changes document of the part that is wrong (`["changes", 2, "path", 1]`), or, for a path that leads nowhere, the edit's own path in the snapshot.

### `runtime-changes-disagree`

Only when the host asks for verification (`verify` with the whole snapshot it believes it now has): the snapshot the changes make is not that snapshot. The host's way of computing changes is wrong, and nothing is applied. The location is the first path, in the snapshot, where they differ.

Location: `input`, the first path that differs.

### Limits

### `runtime-nesting-too-deep`

A render nests more than 512 levels deep, counting each element, each composite's expansion, each fragment, slot and repeated item as one. Nothing is rendered. Before this limit, about 680 nested composites (fewer with several elements to a level) crashed the WebAssembly runtime with an internal error. The limit is not configurable by the host; to stay under it, flatten the composites or split the view. (Decision U12 of the direction documents.)

Location: `source`, the element that would be the 513th level.

### `runtime-snippet-depth`

Snippets are rendered more than 64 levels deep, one inside another. Nothing is rendered. The limit stops a snippet that renders itself with the same arguments, which never ends; a snippet that recurses over data needs at most about 64 levels, since the data is at most 128 levels of record and list. It is not configurable by the host (decision U12). The 512-level limit on all nesting, `runtime-nesting-too-deep`, applies as well.

Location: `source`, the `{@render}` that would be the 65th.

**Fix:** end the recursion with an argument that changes, for example `{#if n > 0}{@render again(n - 1)}{/if}`.

### Internal errors

### `runtime-key-collision`

Two nodes or text runs of one tree would share a key. It can't happen in practice (keys are 128-bit), and it would be a bug to report.

Location: `source`, the later of the two.

---

## CLI errors

### `could not read <file>: <reason>`

Printed by the CLI, not the compiler, when the file (or the `--model` manifest) can't be read: it's missing, it's a directory, you don't have permission, or it isn't valid UTF-8. This is a single line on stderr with no code and no source snippet, even with `--format json`, which then prints nothing on stdout. The exit status is `1`.

---

## Ordering

Syntax errors are reported in source order.

Validation diagnostics are reported element by element, from the root downwards. Within each element they come in this order:

1. duplicate attributes
2. duplicate event bindings
3. mismatched closing tag
4. then the diagnostics of each child, in order

So a parent's mismatched-tag error comes before a warning inside one of its children. The `examples/fixtures/fail/warning-and-error.mprx` fixture shows a warning and an error in one file.

With `--model`, model errors come after all of those, in source order. Two errors that start at the same place keep the order MESH found them in.

## What MESH doesn't diagnose yet

Without `--model`, MESH checks only that a file is well-formed MPRX. Unknown components, props, events, references and commands all pass: catching them needs a component manifest.

With `--model`, element children aren't checked against the component: a component can't yet declare what content it accepts, so any children are allowed. The expressions inside them are still checked.
