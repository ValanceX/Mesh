# Component manifest reference

A **component manifest** is a JSON file that declares the components an MPRX file may use, and what each one accepts. `mesh check --model <MANIFEST>` checks a file against it. This page is the exact format, version 1. For a gentler introduction, with the reasons behind the rules, see [Checking against a component model](../guides/checking-against-a-component-model.md).

The format is also published as a JSON Schema, [`schemas/manifest-v1.schema.json`](../../schemas/manifest-v1.schema.json). Point `"$schema"` at it and most editors will complete and check a manifest as you write it. [`examples/components.json`](../../examples/components.json) is a complete manifest for the repo's examples.

## At a glance

```json
{
  "$schema": "../schemas/manifest-v1.schema.json",
  "version": 1,
  "types": {
    "User": {
      "kind": "record",
      "fields": {
        "name": { "type": { "kind": "string" }, "required": true },
        "avatar": { "type": { "kind": "string" }, "required": false },
        "active": { "type": { "kind": "boolean" }, "required": true }
      }
    }
  },
  "components": {
    "button": {
      "props": {
        "disabled": { "type": { "kind": "boolean" }, "required": false }
      },
      "events": {
        "click": { "payload": { "kind": "any" } }
      },
      "commands": {},
      "scope": {}
    },
    "users-page": {
      "props": {},
      "events": {},
      "commands": {
        "selectUser": {
          "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }]
        }
      },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "compact": { "kind": "boolean" }
      }
    }
  }
}
```

A file checked as the template of `users-page` may read `user` and `compact`, and may handle events with `selectUser(...)`. Wherever it writes `<button>`, that element may have a `disabled` prop and an `on.click` handler.

## The document

| Property | Required | Value |
|---|---|---|
| `"$schema"` | no | Any string. MESH ignores it; editors use it to find the schema. |
| `"version"` | yes | `1`. It's the version of this format, not of MESH, NEXUS or any component library. A manifest with any other version is rejected before anything else in it is read. |
| `"types"` | yes | Named types shared by every component: name → [type](#types). May be `{}`. |
| `"components"` | yes | name → [component](#components). |

No other property is allowed, here or in any object below.

## Components

A component's name is its tag: `"user-card"` is the component `<user-card>` is an instance of. MESH has no built-in elements, so every tag a file uses, `page` and `text` included, must be declared here.

Each component has exactly these four properties. All four are required, even when empty:

| Property | Value | Used when |
|---|---|---|
| `"props"` | name → [field declaration](#field-declarations) | An instance appears in any template: `<button disabled={...}>`. |
| `"events"` | name → [event](#events) | An instance has an `on.<name>` binding: `<button on.click={...}>`. |
| `"commands"` | name → [command](#commands) | Checking this component's *own* template: the commands its handlers may invoke. |
| `"scope"` | name → [type](#types) | Checking this component's own template: every name its expressions may read. |

Props and events describe the component from outside, as other templates use it. Commands and scope describe the inside: what the component's own template may use. A component that is only ever used, never checked as a template, has empty commands and scope; one that is only a template, like a page, has empty props and events.

### Field declarations

Props, and the fields of [records](#types), are declared the same way:

```json
{ "type": <type>, "required": true }
```

Both properties are required: `"required"` has no default. It says whether the prop or field must be written; the type says what its value may be. They are independent:

| Declaration | `"required"` | `"type"` | Must be written? | Its value must be |
|---|---|---|---|---|
| `p: T` | `true` | `T` | yes | a `T` |
| `p?: T` | `false` | `T` | no | a `T` |
| `p: T?` | `true` | `optional` of `T` | yes | a `T`, or a value that may be absent |
| `p?: T?` | `false` | `optional` of `T` | no | a `T`, or a value that may be absent |

So a prop declared `"required": false` may be left out, but a value that may be absent doesn't fit it unless its type is `optional` too. The [guide](../guides/checking-against-a-component-model.md#requiredness-and-absence) explains why.

### Events

```json
{ "payload": <type> }
```

An event with a `"payload"` carries a value of that type, which its handler can pass on as `$event`. An event declared `{}` carries none, and `$event` can't be used in its handler.

### Commands

```json
{ "parameters": [{ "name": "user", "type": <type> }] }
```

`"parameters"` is required, and ordered: an invocation passes its arguments in this order. Every parameter is required, so an invocation passes exactly this many arguments. Parameter names must be distinct; they appear only in messages.

A command returns nothing. It may be invoked only as the whole handler of an `on.` binding, never as a value.

### Scope

```json
{ "user": <type>, "compact": <type> }
```

The scope is every name the component's template may read, and nothing else is: a prop isn't in scope unless the scope lists it too, and there are no global names. The manifest doesn't say where a scope name's value comes from, whether a prop, state or anything else; that's for the runtime.

## Types

Every type is a JSON object with a `"kind"`. There are nine kinds, and no others:

| `"kind"` | Other properties | The type | Written in these docs as |
|---|---|---|---|
| `"string"` | none | a string | `string` |
| `"number"` | none | a number | `number` |
| `"boolean"` | none | `true` or `false` | `boolean` |
| `"null"` | none | the value `null` | `null` |
| `"any"` | none | any value that is present | `any` |
| `"list"` | `"element"`: a type | a list whose elements all have that type | `list<T>` |
| `"record"` | `"fields"`: name → [field declaration](#field-declarations) | an object with exactly those fields | `{ name: string, avatar?: string }` |
| `"named"` | `"name"`: a key of `"types"` | the type declared under that name | `User` |
| `"optional"` | `"type"`: a type | a value of that type, or no value at all (absent) | `T?` |

- **`null` is a value, not absence.** `optional` of `string` accepts a string or absence, not `null`.
- **`any` never includes absence.** A value that may be absent is `optional` of `any`, `any?`.
- **Records are exact.** A record accepts only the fields it declares.
- **Named types are aliases.** `{ "kind": "named", "name": "User" }` means exactly what `"types"` declares as `User`. Two named types with the same definition are the same type.
- **Named types can't be recursive**, directly or through other named types.
- **An `optional` can't wrap a type that is already optional**, whether written directly or through a named type. Write `T?` once.

The [guide](../guides/checking-against-a-component-model.md) and the [language spec](../MPRX-SPEC.md#9-checking-against-a-component-model) say which types fit where.

## Names

Every name must be one MPRX can write where the name is used:

| Name | Rule |
|---|---|
| component | an MPRX tag name: an ASCII letter or `_`, then ASCII letters, digits, `_` or `-` |
| type, prop, event, command, parameter, scope name, record field | an MPRX identifier: an ASCII letter or `_`, then ASCII letters, digits or `_`; and not `true`, `false` or `null` |

Props, events, commands and scope names are separate: each is written in a different place (`name=`, `on.name=`, `name(...)`, and a bare `name`), so one component may use the same name for a prop, an event, a command and a scope name.

## Validation

`mesh check --model` loads and checks the whole manifest before it reads the `.mprx` file. If anything is wrong, it reports every problem it finds, each at its line and column in the manifest, and checks nothing else. Each problem has its own `manifest-*` code, listed in the [diagnostics reference](./diagnostics.md#manifest-errors):

- the JSON itself: `manifest-syntax-error`, `manifest-nesting-too-deep` for arrays and objects nested more than 128 levels deep, and `manifest-duplicate-key` for a key repeated in any object, which most JSON tools would silently accept;
- the version: `manifest-unsupported-version`;
- the format the schema describes: `manifest-invalid-value`, `manifest-missing-property`, `manifest-unknown-property`, `manifest-unknown-kind` and `manifest-invalid-name`;
- rules the schema can't express: `manifest-duplicate-parameter`, `manifest-unknown-type`, `manifest-recursive-type` and `manifest-nested-optional`;
- and once the manifest is valid, `manifest-missing-component` if it doesn't declare the component the file is the template of.

A manifest the JSON Schema accepts can still have the problems in the last two items; `mesh check` is the complete check.

The nesting limit belongs to this MESH, not to the format: it keeps a pathological manifest from exhausting the stack. Write deeply nested types as named types, and a manifest never comes near it.

## Versions

This page describes version 1, the only version this MESH reads. A version 1 manifest keeps meaning what this page says: a change to the format that would change what an existing manifest means, or reject one that is valid today, comes with a new version number. A change that only allows something new, such as a new `"kind"`, may not.
