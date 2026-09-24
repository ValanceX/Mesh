# Checking against a component model

On its own, `mesh check` checks that a file is well-formed MPRX. It can't tell that `<user-card user={usr} />` names something that doesn't exist, because it doesn't know what exists. Give it a **component manifest**, which declares your components and the names each template may use, and it checks what the file *means*: every component, prop, event, name and command must be declared, and every value must fit where it goes.

This matters most for UI you didn't write by hand. A model that generates MPRX can invent a prop, misspell a name, or pass a value that might not be there. With a manifest, each of those is an error with its own code, at the exact place it happens, before anything renders.

This guide explains the ideas, with the reasons behind them. The exact manifest format is in the [manifest reference](../manual/manifest.md), and the exact rules are in the [language spec](../MPRX-SPEC.md#9-checking-against-a-component-model).

## A first check

The repo's examples come with a manifest, [`examples/components.json`](../../examples/components.json), that declares every component they use:

```console
$ mesh check --model examples/components.json examples/users-page.mprx
no errors
```

The examples below come from the repo's test corpus. Each file in `examples/fixtures/check/` is checked as the template of a component named `template`, declared in `examples/fixtures/check/components.json`, and its expected output is next to it. To run one yourself:

```console
$ mesh check --model examples/fixtures/check/components.json --component template examples/fixtures/check/fail/unknown-reference.mprx
```

`template`'s scope has a `user`, but this file wrote `usr`:

```text
error[unknown-reference]: unknown reference "usr": it isn't in the template's scope
 --> examples/fixtures/check/fail/unknown-reference.mprx:1:14
  |
1 | <page title={usr.name} />
  |              ^^^
  = help: did you mean "user"?
```

Each mistake gets one error, at the narrowest place that shows it, with a code that never changes. The misspelled name has an error; reading `.name` from it doesn't get a second one.

## Templates and instances

A manifest declares components, and every `.mprx` file is the **template** of one of them: by default the one named like the file (`users-page.mprx` is `users-page`), or the one `--component` names. The template's component decides what the file may use:

- its **scope**: every name an expression may read, such as `user` in `{user.name}`;
- its **commands**: every command a handler may invoke, such as `selectUser` in `on.click={selectUser($event)}`.

Every element in the file, the root included, is an **instance** of a component, and that component decides what the element may have:

- its **props**: the attributes it accepts, and which ones it must have;
- its **events**: the `on.` bindings it accepts, and what value each one carries.

So in this line of `users-page.mprx`, `user` must be in `users-page`'s scope and `selectUser` among its commands, while `disabled` must be a prop of `button` and `click` one of its events:

```xml
<button disabled={!user.active} on.click={selectUser($event)}>Select</button>
```

Three rules are worth knowing up front:

- **There are no built-in elements.** `page`, `text` and `button` are components like any other, and the manifest must declare them. MESH knows component interfaces, not what a renderer draws.
- **The scope is everything a template can read.** A component's own props aren't in its scope unless the scope lists them too, and there are no global names. Whatever generates the manifest decides what each template can see.
- **Names don't clash across kinds.** Props, events, commands and scope names are written in different places (`name=`, `on.name=`, `name(...)`, a bare `name`), so one component may use the same name for several of them.

## Types

The manifest gives every prop, scope name, record field, command parameter and event payload a type:

| Type | Values |
|---|---|
| `string`, `number`, `boolean` | what they say |
| `null` | the value `null`, and nothing else |
| `any` | any value that is present |
| `list<T>` | a list whose elements are all `T` |
| a record, like `{ name: string, avatar?: string }` | an object with exactly those fields |
| a named type, like `User` | whatever the manifest's `types` declares under that name |
| `T?` | a `T`, or no value at all |

MESH checks every expression's type against where it's used. `{count + 1}` needs `count` to be a `number`, `disabled={...}` needs a `boolean` if that's what `button` declares, and nothing is converted to fit: `"1"` isn't a number, and `0` isn't `false`. The [diagnostics reference](../manual/diagnostics.md#type-mismatch) lists what each place needs.

The rest of this guide is about the three ideas people most often get wrong: requiredness, absence and `any`.

## Requiredness and absence

A prop's declaration says two separate things:

- whether the prop is **required**: must every instance write it?
- its **type**: what may its value be? In particular, if the type is `T?`, the value **may be absent**: there may be no value at all.

They're independent, so there are four combinations. The same four apply to the fields of a record:

| Declaration | Must be written? | Its value must be |
|---|---|---|
| `p: T` | yes | a `T` |
| `p?: T` | no | a `T` |
| `p: T?` | yes | a `T`, or possibly absent |
| `p?: T?` | no | a `T`, or possibly absent |

In the manifest, requiredness is `"required": true` or `false`, and `T?` is an `optional` type. The [manifest reference](../manual/manifest.md#field-declarations) shows both.

**Where absent values come from.** MPRX has no way to write "no value", so an absent value can only come from the manifest: a scope name, a record field or an event payload whose type is `T?`, or a field that isn't required. Reading a field that isn't required gives a value that may be absent, whatever its declared type, because an object may leave it out: `f?: T`, `f: T?` and `f?: T?` all read as `T?`. Only `f: T` reads as `T`.

**A value that may be absent fits only where absence is allowed.** The fixtures' `Layout` record declares `compact?: boolean`, so `layout.compact` is a `boolean?`. That fits a prop declared `compact: boolean?` or `compact?: boolean?`. It doesn't fit `compact?: boolean`, even though that prop may be left out:

```text
error[type-mismatch]: the prop "compact" of component "compact-strict" needs boolean, found boolean?, which may be absent
 --> examples/fixtures/check/fail/conformance-compact-strict.mprx:1:26
  |
1 | <compact-strict compact={layout.compact} />
  |                          ^^^^^^^^^^^^^^
```

A prop that may be left out isn't the same as a prop whose value may be missing. Writing `compact={layout.compact}` passes a value, and if that value turns out to be absent, `compact-strict` would receive a prop that is there but has no value, which its declaration doesn't allow. The message ends with `which may be absent` when that's the only reason the value doesn't fit.

**A required prop must be written, even if its type allows absence.** `avatar` declares `src: string?`: it accepts a value that may be absent, like `user.avatar`, but every `<avatar>` must still say what it passes:

```text
error[missing-required-prop]: component "avatar" requires the prop "src"
 --> examples/fixtures/check/fail/missing-required-prop.mprx:1:2
  |
1 | <avatar size="sm" />
  |  ^^^^^^
```

**Reading a member through a value that may be absent is an error.** There might be nothing to read it from, and MPRX has no optional chaining (`a?.b`):

```text
error[possibly-absent-access]: this value may be absent (its type is User?), so its member "name" can't be read; MPRX has no optional chaining
 --> examples/fixtures/check/fail/possibly-absent-access.mprx:1:8
  |
1 | <text>{maybeUser.name}</text>
  |        ^^^^^^^^^
```

MPRX has no way to test whether a value is present, either. So a value that may be absent can only be passed along to somewhere that accepts absence. If that's too strict for a template, change the manifest: make the scope name's type `T` if it's always there, or give the prop a `T?` type if it can cope with absence.

## Absence isn't `null`

`null` is an ordinary value, with its own type, `null`. **Absence** is the lack of a value. `string?` means "a string, or absent"; it doesn't accept `null`:

```text
error[type-mismatch]: the prop "maybeStr" of component "probe" needs string?, found null
 --> examples/fixtures/check/fail/conformance-null-is-not-absence.mprx:1:18
  |
1 | <probe maybeStr={null} />
  |                  ^^^^
```

For the same reason, comparing with `null` doesn't test for absence. `==` needs its two sides to have a common type, and `string?` and `null` have none:

```text
error[no-common-type]: `==` compares string? with null, which have no common type
 --> examples/fixtures/check/fail/equality-null-is-not-absence.mprx:1:8
  |
1 | <text>{maybeName == null}</text>
  |        ^^^^^^^^^^^^^^^^^
```

Keeping the two apart means a manifest says exactly what a value can be. How a runtime represents absence, whether as JavaScript's `undefined`, a missing key or something else, is up to that runtime; MESH never confuses it with `null`. A prop that accepts a string or `null` needs `any`, since v0.2 has no union types.

## `any` and `any?`

`any` is the escape hatch for a partly typed model: a value of type `any` fits anywhere a present value does, and any present value fits where `any` is expected. Reading a member of an `any` value gives `any`.

But **`any` is never unchecked against absence.** `any` means "some value that is present"; a value that may be absent is `any?`. So the requiredness rules above apply to `any` exactly as to any other type:

| The value | Where it goes | Fits? |
|---|---|---|
| `any` | `string` | yes |
| `any?` | `string` | no: it may be absent |
| `any?` | `string?` | yes |
| `string?` | `any` | no: it may be absent |
| `string?` | `any?` | yes |

```text
error[type-mismatch]: the prop "str" of component "probe" needs string, found any?, which may be absent
 --> examples/fixtures/check/fail/conformance-possibly-absent-any.mprx:1:13
  |
1 | <probe str={maybeAnything} />
  |             ^^^^^^^^^^^^^
```

```text
error[type-mismatch]: the prop "anything" of component "probe" needs any, found string?, which may be absent
 --> examples/fixtures/check/fail/conformance-possibly-absent-to-any.mprx:1:18
  |
1 | <probe anything={maybeName} />
  |                  ^^^^^^^^^
```

Reading a member of an `any?` value is a `possibly-absent-access` error, just as for any other type that may be absent.

## Records are exact

A record type accepts exactly the fields it declares. An object literal given to a record is checked field by field, and each problem is reported where it is. A key the record doesn't declare is an error at the key:

```text
error[unknown-field]: User has no field "nickname"
 --> examples/fixtures/check/fail/unknown-field.mprx:1:43
  |
1 | <probe user={{ name: "Ada", active: true, nickname: "A" }} />
  |                                           ^^^^^^^^
```

A required field that's left out is an error at the object:

```text
error[missing-required-field]: the object is missing the field "active", which User requires
 --> examples/fixtures/check/fail/missing-required-field.mprx:1:14
  |
1 | <probe user={{ name: "Ada" }} />
  |              ^^^^^^^^^^^^^^^
```

Exactness applies to record *types* too: a record with more fields doesn't fit a prop that declares fewer. That's deliberate. An extra field is either a typo or something a generator made up, and silently accepting it would hide both. When two components need to agree on a shape, declare it once in the manifest's `types` and use the named type in both.

## Commands and `$event`

Commands are how a template asks for something to happen, and MESH treats them as actions, not values:

- A command may be invoked only as the whole handler of an `on.` binding: `on.click={selectUser($event)}`. Anywhere else, such as in a prop's value or inside an expression, it's `command-outside-handler`.
- A handler must be a command invocation. `on.click={user.name}` is `handler-not-command`.
- An invocation passes exactly the command's parameters, in order, each of the declared type.
- `$event` is the value the event carries, typed by the event's payload in the element's component. It may appear only in the handler's arguments. If the event carries no value, there's no `$event`:

```text
error[event-has-no-payload]: event "press" of component "button" carries no value, so there is no `$event`
 --> examples/fixtures/check/fail/event-has-no-payload.mprx:1:26
  |
1 | <button on.press={select($event)}>Select</button>
  |                          ^^^^^^
```

## Repairing generated UI

A pipeline that generates MPRX can feed MESH's errors back to the generator until the file checks clean. Use `--format json`, which prints every diagnostic as data:

```console
$ mesh check --format json --model components.json generated.mprx
```

- **Decide by exit status.** `0` means no errors, `1` means errors (or a file that couldn't be read, when stdout is empty).
- **Match on `code`, never on `message`.** Codes are stable; messages are for people and may change.
- **Point at `span`.** It gives byte offsets and lines and columns; `path` says which file, which is the manifest for a `manifest-*` code.
- **Use `suggestions`.** When a name is close to a declared one, the suggestion is the declared name and the span to replace.
- **Fix each diagnostic once.** One mistake gets one diagnostic, so there are no follow-on errors to chase.

The [CLI manual](../manual/mesh-cli.md#json-output) describes the document exactly.

## Where next

- [Manifest reference](../manual/manifest.md): the manifest's exact format and validation.
- [Diagnostics reference](../manual/diagnostics.md#model-errors): every error `--model` can report, and how to fix it.
- [MPRX Language Spec](../MPRX-SPEC.md#9-checking-against-a-component-model): the exact typing and compatibility rules.
