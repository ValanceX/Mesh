# The MESH runtime

> **Status: specified for v0.5; not yet implemented.** This manual is the contract v0.5's runtime, `@valancex/mesh-runtime`, NEXUS's adapter and PORT's renderers are built against (outline: `docs/superpowers/specs/2026-09-25-mesh-v0.5-outline.md`, D3–D6). The keys, handler identifiers and program identities in its examples are illustrative: nothing computes them yet.

The runtime evaluates a **program** of templates (`docs/manual/templates.md`) against a **host**'s values. It produces a **render tree**, which a renderer draws, and turns the events a renderer reports into **command intents** for the host. It is one Rust implementation, compiled natively and to WebAssembly. It implements MPRX's evaluation (§9.7) and the boundary (§9.8) exactly once. It has no I/O, no clock, no randomness and no global state: identical inputs give identical results.

It accepts no MPRX source, only templates, and it resolves no names: every resolution is already in the templates. It uses the model for one thing only, to validate values.

```text
host ── program, model, snapshot ──▶ render ──▶ a render ──▶ its tree ──▶ renderer
                                                    │                        │
host ◀── command intent ◀── dispatch ◀── the render, handler identifier, payload
```

## The two operations

- **render:** a program, a model and a snapshot in. Out: a **render**, or diagnostics.
- **dispatch:** a render, a handler identifier and a payload in. Out: a **command intent**, or diagnostics.

Each returns either its result and no diagnostics, or diagnostics and no result. The runtime has no warnings.

**A render** is one successful result: the render tree, together with the program, model and snapshot it came from. It keeps its snapshot as validated and encoded, so later changes to the host's objects can't reach it.

**The snapshot** is one record of scope values for the program's root template: a scope name to each value (§9.8.4). **A payload** is the value an event carries, or absent.

## The host

A **host** is whatever calls the runtime: NEXUS's adapter, a test host, or any program. It supplies snapshots, keeps renders, relays events, and receives intents.

**The dispatch lifecycle:**
1. The host calls render with a program, a model and a snapshot. The result is a render.
2. The host gives the render's tree to the renderer, and keeps the render.
3. A primitive's event fires. The renderer reports the handler identifier from the tree it drew, and a payload.
4. The host calls dispatch with **the render whose tree the renderer had drawn** when the event fired, and with that identifier and payload.
5. The runtime validates the render's program and model (program validation).
6. It validates the handler identifier against the render's program, then the render's snapshot and the payload (input validation).
7. It finds the handler, and re-derives the scope of each composite on the path to it, by evaluating those occurrences' props against the render's snapshot, exactly as the render did.
8. It evaluates the handler's arguments, applying §9.7's checks.
9. The result is a command intent, or diagnostics.

**Rules of the lifecycle:**
- **Dispatch evaluates against the render's snapshot,** never one supplied separately. So an intent reflects exactly the values the user saw.
- The runtime re-validates what a render contains on every dispatch, because a render passes through the host's hands.
- Only the host knows which render the renderer had drawn. **Keeping renders, and pairing each event with its render, is the host's obligation.**
- Dispatch with a render of an earlier program is valid, and its result is that program's intent.

## The render tree

The render tree is [`schemas/render-v1.schema.json`](../../schemas/render-v1.schema.json). It contains exactly primitive component names, prop names with values from the boundary data model (§9.8.1), text runs as strings, event names each with a handler identifier, and keys. It contains nothing else: no expression, scope name, command, argument, composite name, template, span or `$event`.

- **A node** is a primitive occurrence: `{ "type": "node", "key", "component", "props", "events", "children" }`.
  - `props` maps each written prop to its value. An absent value is an omitted prop; `null` is `null`.
  - `events` maps each event binding's event name to its handler identifier.
  - `children` lists nodes and text runs, in order.
- **Composites never appear.** A composite occurrence is replaced by what its template produced: exactly one node, since a template has one root element.
- **A text run** is `{ "type": "text", "key", "text" }`: the text (§9.7.7) of a maximal sequence of adjacent literal text and interpolation children. A text run is present even when it's empty.

MPRX has no loops and no conditional elements, so **a program's render trees all have the same structure, whatever the snapshot.** Only prop values and text differ.

```json render-v1
{
  "format": "mesh-render",
  "version": 1,
  "root": {
    "type": "node", "key": "kD2_VHca9Ag2dUolhbZRbBg", "component": "page",
    "props": { "title": "Users" },
    "events": {},
    "children": [
      { "type": "text", "key": "kSPIXXSPi5uAsdGkh9bPbbA", "text": "2 users" },
      { "type": "node", "key": "k3oBD74HDIG2MzUDltfcVfw", "component": "avatar",
        "props": { "src": null, "alt": "Ada", "size": 48 },
        "events": { "click": "hDPuYKw885Yg.jn4fVuysv6d-WkiS01rl6A" },
        "children": [] }
    ]
  }
}
```

These are **not** render trees:

An expression left in a prop:

```json render-v1-invalid
{ "format": "mesh-render", "version": 1,
  "root": { "type": "node", "key": "kD2_VHca9Ag2dUolhbZRbBg", "component": "page", "props": {}, "events": {}, "children": [],
    "expression": { "kind": "scope", "name": "user" } } }
```

A template carried along:

```json render-v1-invalid
{ "format": "mesh-render", "version": 1,
  "root": { "type": "node", "key": "kD2_VHca9Ag2dUolhbZRbBg", "component": "page", "props": {}, "events": {}, "children": [],
    "template": "user-card" } }
```

A text run without a key:

```json render-v1-invalid
{ "format": "mesh-render", "version": 1,
  "root": { "type": "node", "key": "kD2_VHca9Ag2dUolhbZRbBg", "component": "page", "props": {}, "events": {},
    "children": [{ "type": "text", "text": "Ada" }] } }
```

### Keys

A key is a node's or text run's structural identity. Keys are:
- **derived only from the program:** from the position, the sequence of child positions from the root template's root, with the component at each step, composites included, so a composite's expansion is part of every key below it;
- **independent of values:** no snapshot or payload affects a key;
- **unique:** no two nodes or text runs in one tree share one;
- **stable within one program:** every render of a program has the same keys at the same positions. They are **not** stable across programs: changing the root or any template (spans included) may change every key. That is intentional, and there is no compatibility guarantee for keys across programs;
- **opaque:** a renderer may compare keys for equality, and must not interpret them;
- **not data identity,** nor application identity. A later design for lists may extend them.

**Encoding.** `H` is SHA-256; strings and counts are written as in the fingerprint (`docs/manual/templates.md`: a string is its 32-bit big-endian UTF-8 byte length and its bytes; a count is 32-bit big-endian). A **path** is the list of steps from the root. Each step is a position (a count), a kind byte, and a component name (a string):

- the root is step `(0, 0x01, its component)`;
- the child at position `i` of a node's `children`, in the render tree, is `(i, 0x01, its component)` for a node, or `(i, 0x02, "")` for a text run;
- a composite occurrence adds two steps: `(i, 0x03, the composite)`, then `(0, 0x01, the component of its template's root element)`, and further expansions nest the same way.

The key is `k` followed by the first 16 bytes of `H(string "mesh-key-v1", the program identity (32 bytes), count of steps, each step)`, in unpadded base64url: 22 characters.

**Why the program identity is in the hash.** The encoding is public. Without the program identity, anyone holding a key could hash guessed paths and component names, such as `user-card`, and confirm a guess, recovering a composite's name, which I13 rules out. With it, confirming a guess needs the program identity, which is a digest of the program's templates. A renderer is given neither: the handler identifiers it sees carry only the identity's first 8 bytes.

**What this does and doesn't provide.** It keeps structural names out of the render tree in a form that can be checked from the tree alone. It is **not** a cryptographic protection of the program:
- keys are unkeyed SHA-256 digests, truncated to 128 bits. They are neither secret nor authenticated, and they don't hide how many nodes there are, or the tree's shape;
- anyone who has the program's templates (for instance, a web page that ships them to the browser beside its renderer) can compute every key and handler identifier, and so match them to names;
- a host must not rely on keys or handler identifiers to keep templates, names or commands confidential, or to authenticate an event. Dispatch validates every handler identifier against the render it's given; that, not secrecy, is what stops a forged one.

**The cost:** keys change whenever the program changes. The tree doesn't say which program it came from, and a renderer mustn't try to infer it (handler identifiers are opaque). The host knows, because it made the render: when it gives the renderer a tree from a different program, it tells the renderer to draw it afresh rather than reconcile it by key.

The runtime checks every tree it builds for duplicate keys, and a collision is an internal error, `runtime-key-collision`. At 128 bits one doesn't happen in practice; the check makes "unique" a guarantee rather than a probability.

### Program identity

A program's **identity** is `H(string "mesh-program-v1", string root, count of templates, then for each template, in code-point order of its component: string component, its canonical digest (32 bytes))`. A template's canonical digest leaves out `compiler` (`docs/manual/templates.md`). The identity changes whenever the root or any template changes, and is never given to a renderer.

### Handler identifiers

A handler identifier names exactly one handler at one node. It is determined by the program's identity, the node's key and the event's name. So it is unique within a program, the same in every render of it, and independent of values. It is not the command, and it contains nothing from which the command or its arguments can be recovered without the program's templates. The same limits apply as for keys: it is opaque, not secret, and dispatch's validation, not its unpredictability, is what makes it safe to accept from a renderer.

**Encoding:** `h`, then the first 8 bytes of the program identity in unpadded base64url (11 characters), then `.`, then the first 16 bytes of `H(string "mesh-handler-v1", program identity (32 bytes), string key, string event name)` in unpadded base64url (22 characters).

The first part lets dispatch tell an identifier from **another program** (`runtime-handler-other-program`) apart from one that names **no handler** in this program (`runtime-unknown-handler`). Dispatch then finds the handler through the node's key and the event, in the template that produced that node.

## The command intent

A **command intent** contains exactly:
- **the command's identity:** its name, and the component whose template declares it. Two components may each declare `selectUser`;
- **one argument per parameter, in parameter order,** each either a value from the boundary data model or **absent**, where the parameter's type is optional.

It is `$defs/intent` in `render-v1.schema.json`:

```json intent
{
  "command": { "component": "user-card", "name": "selectUser" },
  "arguments": [ { "value": { "name": "Ada", "active": true } }, { "absent": true } ]
}
```

`{ "value": null }` is `null`, and `{ "absent": true }` is absent: the two are always distinct.

```json intent-invalid
{ "command": { "component": "user-card", "name": "selectUser" }, "arguments": [ { "value": 1, "absent": true } ] }
```

NEXUS's adapter maps an intent to a NEXUS command (for example, `user-card`'s `selectUser` to `"users.select"`) and its input. That mapping is the adapter's own. **A renderer never receives an intent.**

## Updates

- **A change of values is a new render.** The whole program is evaluated again, and the result is a complete new render tree.
- **Every render of a program has the same structure, keys and handler identifiers,** so a renderer can match a new tree against the one it drew, node by node, by key, and update in place. A tree from a **different** program (a template changed or was added) may have entirely different keys: a renderer draws it afresh, and never matches it against the old tree by key.
- **Finding what changed is the renderer's job,** by comparing prop values and text at equal keys. The runtime does no dependency tracking, no caching of evaluation, and no diffing.

## What renderers and hosts must do

A **renderer**:
- draws each node's component with its props, and each text run's text, **as given**. It computes nothing: it doesn't format numbers, supply a missing prop, or convert values. Values are final;
- reports an event as its handler identifier and payload, and never interprets either;
- compares keys only for equality, and reconciles by key only between trees its host says come from one program;
- surfaces a node of a component it doesn't know, rather than dropping it (a composite whose template a program left out arrives as a node of its name).

A **host**:
- keeps each render while its tree is on screen, and dispatches each event with the render whose tree the renderer had drawn;
- tells its renderer when a new tree comes from a different program than the one drawn, so the renderer draws it afresh instead of reconciling by key;
- shapes its values to the manifest: records are exact, so a record with more fields than its type declares is refused (§9.8.4);
- maps intents to its own commands, and treats diagnostics as errors in its inputs or its program, not as messages for end users.

## Diagnostics

When render or dispatch can't produce its result, it returns diagnostics instead: a **runtime diagnostics document**, [`schemas/runtime-diagnostics-v1.schema.json`](../../schemas/runtime-diagnostics-v1.schema.json). `mesh check-program` prints the same document. Every diagnostic is an error.

```json runtime-diagnostics
{
  "version": 1,
  "diagnostics": [
    { "severity": "error", "code": "runtime-missing-value", "message": "the snapshot has no value for `user`, which is required", "location": { "kind": "input", "path": ["user"] } },
    { "severity": "error", "code": "runtime-value-mismatch", "message": "`users[1].active` is a string, but must be a boolean", "location": { "kind": "input", "path": ["users", 1, "active"] } }
  ]
}
```

### Phases

Each operation runs these phases in order. A phase that reports any diagnostic ends the operation, and no later phase runs.

1. **Program validation.** Its steps run in order, and a step that reports anything ends the phase. Each step reports every problem it finds.
   1. The model must be a valid manifest (the `manifest-*` codes of `docs/manual/diagnostics.md`).
   2. Every template must be well-formed and of a supported format version.
   3. Every template's fingerprint must equal the model's.
   4. The program must satisfy the assembly rules (`docs/manual/templates.md`).
2. **Input validation.**
   - For render: the snapshot. Every mismatch is reported.
   - For dispatch: first the handler identifier. If it's invalid, that is the phase's only diagnostic, because the payload's type depends on the handler. Otherwise, every mismatch in the render's snapshot and the payload is reported.
3. **Evaluation.** Exactly one diagnostic, the first in §9.7.5's order, is reported, and evaluation stops.

### Identity and locations

A diagnostic's identity is its **code** and its **location**. Codes are stable and part of the contract; message wording is not. A location has exactly one of these six forms, and each code always uses the same form (stated in its entry below):

| `kind` | Other properties | Used for |
|---|---|---|
| `model` | `span`: start and end positions in the manifest's text, as the diagnostics document gives them (`byte`, `line`, `column`, `utf16`, `utf16Column`) | manifest diagnostics (`manifest-*`) |
| `program` | | the program as a whole, where there is no template to point to: a missing root template |
| `template` | `index`: the template's 0-based position in the host's list; `component`, when it can be read | template-level validation: a malformed template, an unsupported version, a fingerprint mismatch |
| `source` | `component`, and `span` in template-v1's form (`byte` and `utf16` offsets into the template's source) | every other assembly diagnostic, evaluation diagnostics, and a key collision |
| `input` | `path`: a scope name then field names and list indices, into the snapshot; or `$event` then field names and list indices, into the payload | input diagnostics about the snapshot and payload |
| `handler` | | a handler identifier that dispatch can't accept: there is no meaningful source span or input path to point to |

### Order

- **Program validation:** manifest diagnostics in the manifest's order. Assembly diagnostics: first the one located at the `program`; then those located at a `template`, by `index`; then those at a `source`, by component in code-point order, then by the span's start, then by code.
- **Input validation:** by path. The snapshot's paths come before the payload's. Paths compare segment by segment: names by code point, list indices numerically, and a path comes before its own extensions.
- **Evaluation:** one diagnostic.

### The codes

Every assembly and runtime code, with its location form, what it means and how to fix it, is in the [diagnostics reference](./diagnostics.md#assembly-errors), beside the compiler's codes.

### Every case the v0.5 contract names

| Case (outline, Definition of Done) | Code |
|---|---|
| a broken model | `manifest-*` |
| a malformed template; a template of an unsupported version | `assembly-malformed-template`; `assembly-unsupported-format-version` |
| a template checked against another model (mixed fingerprints) | `assembly-fingerprint-mismatch` |
| two templates for one component; a missing root template | `assembly-duplicate-template`; `assembly-missing-root` |
| an unbound scope name; a binding whose types don't fit; an optional prop bound to a scope name that isn't optional | `assembly-unbound-scope-name`; `assembly-unsound-binding`; `assembly-unsound-binding` |
| a direct or indirect cycle | `assembly-cycle` |
| a composite that declares events; a composite occurrence with children | `assembly-composite-event`; `assembly-composite-children` |
| a missing required scope name; `null` where absence is expected; absence where `null` is expected | `runtime-missing-value`; `runtime-value-mismatch`; `runtime-missing-value` |
| a wrong-kind value at depth | `runtime-value-mismatch` at its path |
| a record with an undeclared field; one missing a field that doesn't read as optional | `runtime-unknown-field`; `runtime-missing-value` |
| an out-of-range number; NaN or an infinity from JavaScript | `runtime-number-out-of-range`; `runtime-non-finite-input` |
| an unpaired surrogate | `runtime-unpaired-surrogate` |
| `undefined` in an array, a hole | `runtime-absent-element` |
| a function, a symbol, a `bigint`, a class instance, a `Map`, a `Date`, a cycle | `runtime-unsupported-value` |
| a payload that doesn't fit; one given for an event with none; a missing one where one is required | `runtime-value-mismatch`; `runtime-unexpected-payload`; `runtime-missing-value` |
| a handler identifier from another program; one naming no handler | `runtime-handler-other-program`; `runtime-unknown-handler` |
| each `any` check failing: an operand, member access, content, a prop, an argument | `runtime-operand-mismatch`; `runtime-not-a-record` or `runtime-missing-member`; `runtime-content-not-text`; `runtime-prop-mismatch`; `runtime-argument-mismatch` |
| a composite prop from `any` that doesn't fit | `runtime-prop-mismatch` |
| a non-finite number reaching a prop, a text run, an argument | `runtime-non-finite-output` |
| an absent list element reaching an output | `runtime-absent-element-output` |
| a list or record interpolated where its static type says so; a literal too large to be finite | `content-not-text`; `number-literal-out-of-range` (check-time, §9.7) |

More example documents, covering every location form. A dispatch with a payload whose field doesn't fit, and with one of the render's snapshot values wrong too (the snapshot's paths come first):

```json runtime-diagnostics
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "runtime-value-mismatch", "message": "`user.active` is a string, but must be a boolean", "location": { "kind": "input", "path": ["user", "active"] } },
  { "severity": "error", "code": "runtime-value-mismatch", "message": "`$event.x` is a string, but must be a number", "location": { "kind": "input", "path": ["$event", "x"] } } ] }
```

```json runtime-diagnostics
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "runtime-non-finite-output", "message": "`1 / count` is Infinity, which can't reach a prop", "location": { "kind": "source", "component": "user-card", "span": { "start": { "byte": 20, "utf16": 20 }, "end": { "byte": 29, "utf16": 29 } } } } ] }
```

```json runtime-diagnostics
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "assembly-missing-root", "message": "the program has no template for its root, `users`", "location": { "kind": "program" } },
  { "severity": "error", "code": "assembly-fingerprint-mismatch", "message": "the template of `user-card` was checked against another model", "location": { "kind": "template", "index": 1, "component": "user-card" } } ] }
```

```json runtime-diagnostics
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "runtime-unknown-handler", "message": "the handler identifier names no handler in this program", "location": { "kind": "handler" } } ] }
```

```json runtime-diagnostics
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "manifest-missing-property", "message": "a prop needs `required`", "location": { "kind": "model", "span": {
    "start": { "byte": 40, "line": 3, "column": 5, "utf16": 40, "utf16Column": 5 },
    "end": { "byte": 60, "line": 3, "column": 25, "utf16": 60, "utf16Column": 25 } } } } ] }
```

These are **not** runtime diagnostics documents. The runtime has no warnings:

```json runtime-diagnostics-invalid
{ "version": 1, "diagnostics": [
  { "severity": "warning", "code": "runtime-missing-value", "message": "…", "location": { "kind": "input", "path": ["user"] } } ] }
```

An input location without its path:

```json runtime-diagnostics-invalid
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "runtime-missing-value", "message": "…", "location": { "kind": "input" } } ] }
```

A location of a form that doesn't exist (the six forms are all there are):

```json runtime-diagnostics-invalid
{ "version": 1, "diagnostics": [
  { "severity": "error", "code": "runtime-unknown-handler", "message": "…", "location": { "kind": "event", "name": "click" } } ] }
```
