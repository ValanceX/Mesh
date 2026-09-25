# Templates and programs

> **Status: specified for v0.5; not yet implemented.** This manual is the contract v0.5's compiler and runtime are built against (outline: `docs/superpowers/specs/2026-09-25-mesh-v0.5-outline.md`, D1 and D7). Until they ship, nothing emits or reads a template.

A **template** is one component's MPRX, checked clean against a component model and compiled. A **program** is a root component and a set of templates. The MESH runtime renders a program against the host's values (`docs/manual/runtime.md`).

MPRX goes through three stages: **source syntax → semantic resolution → runtime evaluation.** A template is the product of the first two and holds nothing from the third. Every name in it is resolved, and it holds no values.

## What a template contains

- **Its format version,** `1`: the version of its serialized structure, and its only compatibility version.
- **Its component,** the one whose template it is.
- **The model fingerprint** of the model it was checked against ([below](#the-model-fingerprint)).
- **Its compiler version,** for provenance and debugging only. It never affects whether the template runs.
- **Its element tree, as checked:**
  - only the attributes and event bindings §3's duplicate resolution keeps (the last of each name);
  - no whitespace-only text (§3);
  - every attribute, event binding, child and expression.
- **For every name, what it resolved to** ([below](#names)).
- **Literal values:** each number literal as the binary64 value it converts to (§9.7.3), and each quoted attribute value and piece of literal text as its string.
- **Spans,** as byte and UTF-16 offsets, for diagnostics and developer tools. Nothing depends on them for meaning.

A template does **not** contain:
- the value of any scope name, prop or payload;
- any evaluated prop or text;
- anything that depends on another template.

A reference names the scope declaration it reads. Its value is supplied only at runtime: by the snapshot, for the root template, or by the occurrence's props, for a composite ([Programs](#programs)).

**What belongs to a template, and what to a program:**

| Information | Where it lives |
|---|---|
| format version | each template |
| component identity | each template (its own); each occurrence (what it's an instance of) |
| model fingerprint | each template. A program has none of its own: it is the one all its templates share. |
| composite or primitive | the program only: an occurrence is composite exactly when the program has a template for its component. A template never records it. |
| template lookup | the program only, by component. The model is never consulted. |
| root component | the program only |
| cross-template references | none inside a template: an occurrence names a component, and the program supplies the template, if there is one |
| handler lookup | each handler lives in its template; the runtime finds it through its handler identifier (`docs/manual/runtime.md`) |

## The format

The format is [`schemas/template-v1.schema.json`](../../schemas/template-v1.schema.json). It is JSON, under the same promise as the diagnostics document: a later MESH may add properties without changing `version`, and removing one or changing its meaning is a new version. A reader must ignore properties it doesn't know.

| Property | Holds |
|---|---|
| `format` | always `"mesh-template"`, so a stray JSON document is refused as malformed rather than as a version mismatch |
| `version` | `1` |
| `component` | the component whose template this is |
| `fingerprint` | `sha256:` and 64 lowercase hexadecimal digits |
| `compiler` | the compiler's version, such as `"0.5.0"` |
| `root` | the root element |

- **An element** is `{ "component", "props", "events", "children", "span" }`.
  - `props` lists the kept attributes in source order, each `{ "prop", "value", "span" }`, where `value` is an expression. A quoted attribute is a string literal.
  - `events` lists the kept event bindings in source order, each `{ "event", "command", "arguments", "span" }`.
  - `children` lists the children in order, each `{ "kind": "text", "value", "span" }`, `{ "kind": "expression", "expression" }` or `{ "kind": "element", "element" }`.
- **A handler is not an expression.** A command can appear only as a handler, and `$event` only inside a handler's arguments (§9.5). The schema makes both structural: `arguments` are `argument` expressions, which may contain `{ "kind": "event" }` at any depth, and every other expression is an `expression`, which can't. A template that breaks either rule is malformed.
- **Expressions** each have a `kind` and a `span`:

  | `kind` | Other properties | Is |
  |---|---|---|
  | `literal` | `value`: a string, a finite number (never `-0`), a boolean or `null` | a literal, or a quoted attribute value |
  | `scope` | `name` | a reference to a scope declaration of the template's component |
  | `member` | `object`, `field` | `object.field` |
  | `unary` | `operator` (`not`, `negate`), `operand` | `!x`, `-x` |
  | `binary` | `operator`, `left`, `right` | `operator` is one of `add`, `subtract`, `multiply`, `divide`, `remainder`, `equal`, `not-equal`, `less`, `less-equal`, `greater`, `greater-equal`, `and`, `or` |
  | `conditional` | `condition`, `consequent`, `alternate` | `c ? a : b` |
  | `list` | `elements` | a list literal |
  | `record` | `fields`: `[{ "name", "value", "span" }]` | a record literal; shadowed keys (§9.4) are omitted |
  | `event` | | `$event` (in `argument` expressions only) |

- **Numbers** are JSON numbers holding exactly the binary64 value. A writer writes text that reads back as that value; a reader must convert with correct rounding (in Rust, `serde_json` with `float_roundtrip`; JavaScript's `JSON.parse` already does).
- **A span** is `{ "start": { "byte", "utf16" }, "end": { "byte", "utf16" } }`, offsets into the source as given, counted as the diagnostics document counts them: `byte` in UTF-8 bytes, a byte-order mark included, and `utf16` in UTF-16 code units of the same text.

### Names

A template holds each name as the declaration it resolved to, so it needs no separate table of resolutions:

- an element's `component` is a component of the model;
- a prop's `prop` is a prop of that element's component, and an event's `event` is an event of it;
- a handler's `command` is a command of the **template's own** component. So a command's identity is the pair (`component` of the template, `command`) (D4);
- a `scope` expression's `name` is a scope declaration of the template's own component;
- `$event` is the payload of the event its handler is bound to.

The runtime checks every name against its model once, when it validates the program. A name the model doesn't declare, with the kind the template gives it, makes the template malformed (`assembly-malformed-template`).

### An example

For this model:

```json manifest
{
  "version": 1,
  "types": {
    "User": {
      "kind": "record",
      "fields": {
        "name": { "type": { "kind": "string" }, "required": true },
        "avatar": { "type": { "kind": "string" }, "required": false }
      }
    }
  },
  "components": {
    "avatar": {
      "props": {
        "src": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": true },
        "alt": { "type": { "kind": "string" }, "required": true }
      },
      "events": { "click": {} },
      "commands": {},
      "scope": {}
    },
    "user-card": {
      "props": { "user": { "type": { "kind": "named", "name": "User" }, "required": true } },
      "events": {},
      "commands": {
        "selectUser": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] }
      },
      "scope": { "user": { "kind": "named", "name": "User" } }
    }
  }
}
```

the template of `user-card` whose source is

```xml
<avatar src={user.avatar} alt={user.name} on.click={selectUser(user)} />
```

is:

```json template-v1
{
  "format": "mesh-template",
  "version": 1,
  "component": "user-card",
  "fingerprint": "sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf",
  "compiler": "0.5.0",
  "root": {
    "component": "avatar",
    "props": [
      {
        "prop": "src",
        "value": {
          "kind": "member",
          "object": {
            "kind": "scope",
            "name": "user",
            "span": {
              "start": {
                "byte": 13,
                "utf16": 13
              },
              "end": {
                "byte": 17,
                "utf16": 17
              }
            }
          },
          "field": "avatar",
          "span": {
            "start": {
              "byte": 13,
              "utf16": 13
            },
            "end": {
              "byte": 24,
              "utf16": 24
            }
          }
        },
        "span": {
          "start": {
            "byte": 8,
            "utf16": 8
          },
          "end": {
            "byte": 25,
            "utf16": 25
          }
        }
      },
      {
        "prop": "alt",
        "value": {
          "kind": "member",
          "object": {
            "kind": "scope",
            "name": "user",
            "span": {
              "start": {
                "byte": 31,
                "utf16": 31
              },
              "end": {
                "byte": 35,
                "utf16": 35
              }
            }
          },
          "field": "name",
          "span": {
            "start": {
              "byte": 31,
              "utf16": 31
            },
            "end": {
              "byte": 40,
              "utf16": 40
            }
          }
        },
        "span": {
          "start": {
            "byte": 26,
            "utf16": 26
          },
          "end": {
            "byte": 41,
            "utf16": 41
          }
        }
      }
    ],
    "events": [
      {
        "event": "click",
        "command": "selectUser",
        "arguments": [
          {
            "kind": "scope",
            "name": "user",
            "span": {
              "start": {
                "byte": 63,
                "utf16": 63
              },
              "end": {
                "byte": 67,
                "utf16": 67
              }
            }
          }
        ],
        "span": {
          "start": {
            "byte": 42,
            "utf16": 42
          },
          "end": {
            "byte": 69,
            "utf16": 69
          }
        }
      }
    ],
    "children": [],
    "span": {
      "start": {
        "byte": 0,
        "utf16": 0
      },
      "end": {
        "byte": 72,
        "utf16": 72
      }
    }
  }
}
```

Each of these is **not** a template, and the runtime refuses it as malformed:

A command as a prop value:

```json template-v1-invalid
{ "format": "mesh-template", "version": 1, "component": "user-card",
  "fingerprint": "sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf", "compiler": "0.5.0",
  "root": { "component": "avatar", "events": [], "children": [], "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } },
    "props": [{ "prop": "alt", "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } },
      "value": { "kind": "command", "command": "selectUser", "arguments": [], "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } } } }] } }
```

`$event` outside a handler:

```json template-v1-invalid
{ "format": "mesh-template", "version": 1, "component": "user-card",
  "fingerprint": "sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf", "compiler": "0.5.0",
  "root": { "component": "avatar", "events": [], "children": [], "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } },
    "props": [{ "prop": "alt", "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } },
      "value": { "kind": "event", "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } } } }] } }
```

No fingerprint:

```json template-v1-invalid
{ "format": "mesh-template", "version": 1, "component": "user-card", "compiler": "0.5.0",
  "root": { "component": "avatar", "props": [], "events": [], "children": [], "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } } } }
```

A version that isn't a number (a supported number other than `1` is refused as unsupported, not as malformed):

```json template-v1-invalid
{ "format": "mesh-template", "version": "1", "component": "user-card",
  "fingerprint": "sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf", "compiler": "0.5.0",
  "root": { "component": "avatar", "props": [], "events": [], "children": [], "span": { "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } } } }
```

## Compatibility

Exactly two things decide whether a template runs under a runtime and a model:
1. the runtime supports its `version`;
2. its `fingerprint` equals the fingerprint of the model the runtime is given.

A template runs under any runtime that supports its format version, whatever compiler version produced it. Recompiling is the remedy for a changed model.

## The model fingerprint

The fingerprint is determined by the model's **meaning**, and by nothing else.

**What counts:**
- the set of components;
- for each component: its props (name, type, requiredness); its events (name, and payload type or none); its commands (name, and each parameter's type, in order); and its scope (name and type);
- types as their full structural expansion, since named types are structural (§9.3).

**What doesn't count:**
- JSON formatting, whitespace and the spelling of numbers;
- the order of object keys (every declaration map is a set);
- the `$schema` property and the manifest format's `version`;
- the names of named types, and named types nothing uses;
- parameter names, which appear only in messages.

### How it is computed

`H(bytes)` is SHA-256. A **string** is written as its UTF-8 byte length, a 32-bit big-endian unsigned integer, then its UTF-8 bytes. A **count** is a 32-bit big-endian unsigned integer. A **flag** is one byte, `0x00` or `0x01`. Wherever names are listed, they are in code-point order, which is the order of their UTF-8 bytes.

**A type's digest,** `D(T)`, is `H` of one tag byte followed by the type's parts:

| Type | Bytes hashed |
|---|---|
| `string` | `0x01` |
| `number` | `0x02` |
| `boolean` | `0x03` |
| `null` | `0x04` |
| `any` | `0x05` |
| `list<U>` | `0x06`, `D(U)` |
| `U?` | `0x07`, `D(U)` |
| record | `0x08`, the count of fields, then for each field: its name (a string), its requiredness (a flag, `0x01` for required), `D(its type)` |
| a named type | nothing of its own: `D(named)` is `D(its definition)` |

A digest is computed once per named type and reused, so the fingerprint costs time linear in the manifest's size, however often a named type is used, and equals what hashing the full expansion would give.

**A component's digest,** `C`, is `H` of:
1. `0x10`;
2. the count of props, then for each prop: its name, its requiredness (a flag), `D(its type)`;
3. the count of events, then for each event: its name, then `0x00` if it has no payload, or `0x01` and `D(its payload)`;
4. the count of commands, then for each command: its name, the count of its parameters, then `D(each parameter's type)`, in parameter order;
5. the count of scope names, then for each: its name, `D(its type)`.

**The fingerprint** is `H` of the string `mesh-model-v1`, the count of components, then for each component its name and `C`. It is written `sha256:` followed by the digest as 64 lowercase hexadecimal digits.

### Worked example

For the model in [the example above](#an-example):

| Digest | Value |
|---|---|
| `D(string)` | `4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a` |
| `D(string?)` | `6b8366fb73914d2bc5dfea5fa2a4bf0d4af1fd16fbf39aa0b6bb20a2862a7467` |
| `D(User)` | `01f3f4bcfe2556feadd3a7e8c22a3e6583de7a286784cebbaafdc6ef905881cd` |
| `C(avatar)` | `def707e7cf8cb961c610deef89193a2f4dafec33b6011d87886f1d7bfc2f865a` |
| `C(user-card)` | `1c8d27ec9cc415375cf591fbf503a51a5dc5b8c9789ce84be37364576f3ac2e0` |
| fingerprint | `sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf` |

The same model with `User` renamed `Person`, the parameter renamed, every object's keys reversed, `$schema` added and an unused named type added has the same fingerprint. With `alt` no longer required, it is `sha256:bdc6b928c3f80266293a5d2a980c92354364a480664e10a9886325808b653822`.

## A template's canonical digest

Some identities are computed from templates (a program's identity, `docs/manual/runtime.md`). They use a template's **canonical digest**: SHA-256 of the template's canonical JSON, **without its `compiler` property**, so that recompiling with another compiler version, to the same template, changes nothing.

The canonical JSON is the JSON Canonicalization Scheme (RFC 8785) applied to the template as given, unknown properties included:
- object members sorted by their names' UTF-16 code units;
- no whitespace;
- strings with RFC 8785's minimal escaping;
- numbers written as §9.7.7.1 writes them. That agrees with RFC 8785's ECMAScript serialization wherever ECMAScript determines the digits, and fixes the last digit where it doesn't.

Spans are part of it: a template recompiled from reformatted source is a different template.

## Programs

A **program** is a root component and a set of templates, at most one per component.

- Its model fingerprint is the one its templates share.
- It has no document of its own. A host gives the compiler's program check (`mesh check-program`) and the runtime its parts: the root's name, and the list of templates.
- Every template in the program is subject to the assembly rules, whether or not the root reaches it. A template the root doesn't reach is valid, and ignored when rendering.

### Composite or primitive

Whether an occurrence is composite or primitive is decided **by the program, and only by the program**:

- an occurrence whose component has a template in the program is a **composite occurrence**, and the runtime expands it by evaluating that template in its place;
- any other occurrence is a **primitive occurrence**, and becomes a node of the render tree (`docs/manual/runtime.md`);
- **a component with no template in the program is a primitive**, always. That isn't an error, whatever the model declares for it. The manifest has no say: nothing in it marks a component as either.

So leaving a composite's template out of a program silently makes it a primitive, and a renderer receives a node for it. A renderer should surface a node of a component it doesn't know, never drop it.

### Binding a composite's scope

A composite's template reads its own scope names. Those names are bound by the composite occurrence's props. For every composite `C` used as a composite occurrence in any template of the program:

1. every scope name `s` of `C` must be bound by a prop of `C` with the same name;
2. the binding must be sound. With the prop declared as type `P`, and `s` of type `S`:
   - if the prop is required, `is_assignable(P, S)` must hold (§9.3);
   - if it's optional, `is_assignable(P?, S)` must hold (or `is_assignable(P, S)`, when `P` is already optional), because an unwritten prop binds absence.

At runtime, each written prop's value is evaluated in the enclosing template and checked against the prop's declared type (§9.7.6). An unwritten prop binds absent. A prop with no scope name of the same name is evaluated, and then not used.

Only the root template's scope comes from the snapshot. The root component's own props play no part. The root is exempt from the binding rules, unless it is also used as a composite occurrence somewhere, which is then a cycle (rule 5).

**Nesting.** A composite's template may contain composite occurrences, to any depth, as long as the program has no cycle.

**Handlers inside a composite** invoke that composite's own commands: a `user-card` template's `on.click={selectUser(user)}` produces `user-card`'s `selectUser`. v0.5 has no composite events: a composite can't raise events of its own.

### The assembly rules

A program must satisfy these rules. Each broken rule is an **assembly error**. The runtime checks them before it evaluates anything (its program validation, `docs/manual/runtime.md`), and so does the compiler's program check, with the same codes and locations.

| # | Rule | Code | Program validation step |
|---|---|---|---|
| 1 | At most one template per component. | `assembly-duplicate-template` | 4 |
| 2 | The root component has a template in the program. | `assembly-missing-root` | 4 |
| 3a | Every template is well-formed: valid against `template-v1`, and every name in it declared by the model with the kind it's given ([Names](#names)). | `assembly-malformed-template` | 2 |
| 3b | Every template has a format version the runtime supports. | `assembly-unsupported-format-version` | 2 |
| 3c | Every template has the fingerprint of the given model. So all of a program's templates share one. | `assembly-fingerprint-mismatch` | 3 |
| 4a | Every scope name of every composite used as an occurrence is bound by a prop of the same name. | `assembly-unbound-scope-name` | 4 |
| 4b | Every such binding is sound. | `assembly-unsound-binding` | 4 |
| 5 | **No cycles:** no composite expands itself, directly or through others. Every composite occurrence on a cycle is reported. | `assembly-cycle` | 4 |
| 6 | **No composite declares events** in the model. | `assembly-composite-event` | 4 |
| 7 | **No composite occurrence has children,** text or elements. Whitespace-only text isn't a child (§3). | `assembly-composite-children` | 4 |

Program validation's steps run in order, and a step that reports anything ends validation. Step 1 is the model itself (a manifest error). Within a step, every problem is reported.

**Where each is reported.**
Each rule has one location form, of the six `docs/manual/runtime.md` defines:
- Rules 3a, 3b and 3c are located at the **`template`**: its position in the list the host gave, with its component when it can be read, because a malformed template may have no readable component.
- Rule 2 is located at the **`program`**: there is no root template to point to.
- The others are located at a **`source`**, a template's component and a span in it: rule 1 at the root element of each template of the component after the first; rules 4a, 4b, 5 and 7 at the composite occurrence; rule 6 at every occurrence of the composite.

**Order.** Assembly errors are listed with the one located at the `program` first (rule 2); then those located at a `template`, by position; then those located at a `source`, by the template's component in code-point order, then by the span's start, then by code.

### Examples

With a model in which `user-card` has the prop `user: User` (required), the scope name `user: User` and the command `selectUser(user: User)`, and `page` and `avatar` are components with no template:

| Program | Result |
|---|---|
| root `page`; templates: `page`'s `<page><user-card user={first} /></page>`, `user-card`'s `<avatar src={user.avatar} alt={user.name} on.click={selectUser(user)} />` | valid: `user-card` is a composite, `page` the root, `avatar` a primitive |
| the same, with a second template for `user-card` | `assembly-duplicate-template` (rule 1) |
| root `page`, with no template for `page` | `assembly-missing-root` (rule 2) |
| a template compiled against a model where `avatar`'s `alt` isn't required | `assembly-fingerprint-mismatch` (rule 3c) |
| a model where `user-card`'s prop is named `person` | `assembly-unbound-scope-name` at the occurrence (rule 4a) |
| a model where `user-card`'s prop `user` isn't required, and its scope name `user` isn't optional | `assembly-unsound-binding` (rule 4b): an unwritten prop would bind absent |
| `user-card`'s template containing `<user-card user={user} />` | `assembly-cycle` (rule 5) |
| a model where `user-card` declares an event `select` | `assembly-composite-event` (rule 6) |
| `<user-card user={first}>Ada</user-card>` | `assembly-composite-children` (rule 7) |
| `<user-card user={first}>  </user-card>` | valid: whitespace-only text isn't a child |
| the valid program plus a template for `badge`, which nothing uses | valid: the unreached template is checked and ignored |
| the valid program without `user-card`'s template | valid: `user-card` is then a primitive, and the render tree holds a `user-card` node |

### Assembly errors and runtime errors

- Every rule above is decided before evaluation, from the program and the model alone. So each is an assembly error, and nothing is rendered.
- During evaluation, composition can fail only through §9.7's runtime checks. The one specific to composition is a composite prop whose value, coming from an `any`, doesn't fit the prop's declared type: that is an evaluation error (`runtime-prop-mismatch`).
