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
- **stable:** every render of a program has the same keys at the same positions;
- **opaque:** a renderer may compare keys for equality, and must not interpret them;
- **not data identity,** nor application identity. A later design for lists may extend them.

**Encoding.** `H` is SHA-256; strings and counts are written as in the fingerprint (`docs/manual/templates.md`: a string is its 32-bit big-endian UTF-8 byte length and its bytes; a count is 32-bit big-endian). A **path** is the list of steps from the root. Each step is a position (a count), a kind byte, and a component name (a string):

- the root is step `(0, 0x01, its component)`;
- the child at position `i` of a node's `children`, in the render tree, is `(i, 0x01, its component)` for a node, or `(i, 0x02, "")` for a text run;
- a composite occurrence adds two steps: `(i, 0x03, the composite)`, then `(0, 0x01, the component of its template's root element)`, and further expansions nest the same way.

The key is `k` followed by the first 16 bytes of `H(string "mesh-key-v1", the program identity (32 bytes), count of steps, each step)`, in unpadded base64url: 22 characters.

The program identity is part of the hash so that a key can't be checked against guessed names: without the program's templates, nobody can compute a key, and so nobody can recover a composite's name from one. As a result, keys change whenever the program changes (any template, spans included). They are stable only within one program, which is what D4 requires. The runtime checks every tree it builds for duplicate keys, and a collision is an internal error, `runtime-key-collision`: at 128 bits it doesn't happen, and the check makes "unique" a guarantee rather than a probability.

### Program identity

A program's **identity** is `H(string "mesh-program-v1", string root, count of templates, then for each template, in code-point order of its component: string component, its canonical digest (32 bytes))`. A template's canonical digest leaves out `compiler` (`docs/manual/templates.md`). The identity changes whenever the root or any template changes, and is never given to a renderer.

### Handler identifiers

A handler identifier names exactly one handler at one node. It is determined by the program's identity, the node's key and the event's name. So it is unique within a program, the same in every render of it, and independent of values. It is not the command, and it contains nothing from which the command or its arguments can be recovered.

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
- **Every render of a program has the same structure, keys and handler identifiers,** so a renderer can match a new tree against the one it drew, node by node, by key, and update in place.
- **Finding what changed is the renderer's job,** by comparing prop values and text at equal keys. The runtime does no dependency tracking, no caching of evaluation, and no diffing.

## What renderers and hosts must do

A **renderer**:
- draws each node's component with its props, and each text run's text, **as given**. It computes nothing: it doesn't format numbers, supply a missing prop, or convert values. Values are final;
- reports an event as its handler identifier and payload, and never interprets either;
- compares keys only for equality;
- surfaces a node of a component it doesn't know, rather than dropping it (a composite whose template a program left out arrives as a node of its name).

A **host**:
- keeps each render while its tree is on screen, and dispatches each event with the render whose tree the renderer had drawn;
- shapes its values to the manifest: records are exact, so a record with more fields than its type declares is refused (§9.8.4);
- maps intents to its own commands, and treats diagnostics as errors in its inputs or its program, not as messages for end users.
