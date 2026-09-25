# Diagnostics reference

This page lists every diagnostic MESH can produce: its code, what triggers it, how serious it is, and how to fix it. For the output formats, see the CLI manual: [human](./mesh-cli.md#diagnostic-format) and [JSON](./mesh-cli.md#json-output).

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
error[manifest-unknown-property]: prop "size" can't have the property "default"
  --> examples/fixtures/manifest/fail/unknown-property.json:15:68
   |
15 |         "size": { "type": { "kind": "string" }, "required": false, "default": "md" }
   |                                                                    ^^^^^^^^^
```

**Fix:** remove the property. The manifest describes interfaces only: there are no default values, and a type has only the properties of its kind.

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

A type's `"kind"` isn't one of `string`, `number`, `boolean`, `null`, `any`, `list`, `record`, `named` or `optional`.

```text
error[manifest-unknown-kind]: unknown type kind "integer"; expected string, number, boolean, null, any, list, record, named or optional
  --> examples/fixtures/manifest/fail/unknown-kind.json:18:28
   |
18 |         "count": { "kind": "integer" }
   |                            ^^^^^^^^^
```

`void` and `nothing` are types MESH uses internally, and a manifest can't write them either.

**Fix:** use one of the kinds above. For a number, use `number`: there's no separate integer type.

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

A command is invoked with more or fewer arguments than it declares parameters. Every parameter is required. Points at the whole invocation.

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

A member access reads a field of a value that may be absent: its type is optional (`T?`), `any?` included. MPRX has no optional chaining and no presence test, so there is no way to read it safely. Points at the value.

```text
error[possibly-absent-access]: this value may be absent (its type is User?), so its member "name" can't be read; MPRX has no optional chaining
 --> examples/fixtures/check/fail/possibly-absent-access.mprx:1:8
  |
1 | <text>{maybeUser.name}</text>
  |        ^^^^^^^^^
```

Reading a field that is itself optional is fine: `user.avatar`, where `avatar` is declared `"required": false`, has type `string?`. It's reading *through* it, as in `user.avatar.length`, that fails.

**Fix:** give the template a value that is always present, for example a scope name typed `User` rather than `User?`.

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

Two types that must have a common type don't: the operands of `==` or `!=`, the two branches of `c ? a : b`, or the elements of an array. Points at the comparison or conditional, or at the first array element that doesn't fit with the ones before it.

```text
error[no-common-type]: `==` compares string with number, which have no common type
 --> examples/fixtures/check/fail/no-common-type.mprx:1:8
  |
1 | <text>{name == 1}</text>
  |        ^^^^^^^^^
```

Where a type is expected, such as a prop's value, an array literal is checked against it element by element and a conditional branch by branch, so they need no common type there: `[{ name: "Ada", active: true }, { name: "Bo", active: false, avatar: "bo.png" }]` fits a `list<User>` prop, and a wrong element or branch is a `type-mismatch` at itself.

Equality is strict, with no conversions: a string never equals a number, so comparing them is a mistake rather than `false`. Two types have a common type when they are the same, when one is `any`, or when one is only optional where the other isn't: `string` and `string?` have the common type `string?`. `null` is a value, not absence, so `maybeName == null` compares `string?` with `null` and has no common type. An empty array `[]` fits with any list.

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
