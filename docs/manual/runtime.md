# The MESH runtime

The runtime evaluates a **program** of templates (`docs/manual/templates.md`) against a **host**'s values. It produces a **render tree**, which a renderer draws, and turns the events a renderer reports into **command intents** for the host. It is one Rust implementation, compiled natively (the `mesh-runtime` crate) and to WebAssembly (`@valancex/mesh-runtime`). It implements MPRX's evaluation (§9.7) and the boundary (§9.8) exactly once. It has no I/O, no clock, no randomness and no global state: identical inputs give identical results.

It accepts no MPRX source, only templates, and it resolves no names: every resolution is already in the templates. It uses the model for one thing only, to validate values.

```text
host ── program, model, snapshot ──▶ render ──▶ a render ──▶ its tree ──▶ renderer
                                                    │                        │
host ◀── command intent ◀── dispatch ◀── the render, handler identifier, payload
```

## The operations

- **render:** a program, a model and a snapshot in. Out: a **render**, or diagnostics.
- **dispatch:** a render, a handler identifier and a payload in. Out: a **command intent**, or diagnostics.
- **update:** a render and a new snapshot of the same program in. Out: the new **render** and the **patches** from the previous tree to its tree, or diagnostics ([Update](#update)).

Render and dispatch are the two the lifecycle below is about; `declaredEvents` (a program's declared events, below) is a fourth, and takes no snapshot.

Each returns either its result and no diagnostics, or diagnostics and no result. The runtime has no warnings.

**A render** is one successful result: the render tree, together with the program, model and snapshot it came from. It keeps its snapshot as validated and encoded, so later changes to the host's objects can't reach it.

**The snapshot** is one record of scope values for the program's root template: a scope name to each value (§9.8.4). **A payload** is the value an event carries, or absent.

## The host

A **host** is whatever calls the runtime: NEXUS's adapter, a test host, or any program. It supplies snapshots, keeps renders, relays events, and receives intents.

**The dispatch lifecycle:**
1. The host calls render with a program, a model and a snapshot. The result is a render.
2. The host gives the render's tree to the renderer, and keeps the render.
3. An interaction occurs on the drawn tree. The renderer resolves it to at most one binding (§9.9) and reports that binding's handler identifier, from the tree it drew, with the event's payload. An interaction that resolves to nothing is not reported.
4. The host calls dispatch with **the render whose tree the renderer had drawn** when the event fired, and with that identifier and payload.
5. The runtime validates the render's program and model (program validation).
6. It validates the handler identifier against the render's program, and, when the program has a conditional, against the render's tree: an identifier whose node isn't in it is unknown (§9.10.7). Then it validates the render's snapshot and the payload (input validation).
7. It finds the handler, and re-derives the scope of each composite on the path to it, by evaluating those occurrences' props against the render's snapshot, exactly as the render did.
8. It evaluates the handler's arguments, applying §9.7's checks.
9. The result is a command intent, or diagnostics.

**Rules of the lifecycle:**
- **Dispatch evaluates against the render's snapshot,** never one supplied separately. So an intent reflects exactly the values the user saw.
- The runtime re-validates what a render contains on every dispatch, because a render passes through the host's hands.
- Only the host knows which render the renderer had drawn. **Keeping renders, and pairing each event with its render, is the host's obligation.**
- Dispatch with a render of an earlier program is valid, and its result is that program's intent.

## Calling the runtime

**From Rust,** `mesh_runtime::render(&program, model, &snapshot)` returns a `Render` or the diagnostics, and `mesh_runtime::dispatch(&render, handler, payload)` returns an `Intent` or the diagnostics. A `Program` is the root's name and the templates' texts; the model is the manifest's text; a snapshot is a `HostRecord` (`HostRecord::from_json` reads one). A host that keeps a render's program, model and snapshot rather than the `Render` itself calls `mesh_runtime::dispatch_from` with them, which validates them all again, as dispatch always does. `mesh_runtime::to_json` writes diagnostics as the document below.

**From JavaScript,** `@valancex/mesh-runtime` has the same operations (`render`, `dispatch` and `update`, and `declaredEvents`, below): `render({ program: { root, templates }, model, snapshot })` resolves to `{ render }` or `{ diagnostics }`, and `dispatch(render, handler, payload?)` to `{ intent }` or `{ diagnostics }`. `update(render, snapshot)` resolves to `{ render, patches }` or `{ diagnostics }` (see [Update](#update)), and `declaredEvents({ program, model })` resolves to a program's [declared events](#the-declared-events) or `{ diagnostics }`. `render.tree` is the render tree, frozen. Its [README](../../packages/mesh-runtime/README.md) has the details. What a JavaScript host needs to know about its values (§9.8.6):
- The package **encodes, and never judges.** (The one tree it reads is `update`'s: it applies the patches the module returns to the previous render's tree, as a renderer would, moving parts by key and reading no value, so that an update costs what changed and not the size of the tree. The package's review test names that file as its only exception.) Every value reaches the runtime, which reports what it can't accept: NaN, the infinities, a hole or `undefined` in an array, an unpaired surrogate, a `Map`, a `Date`, a class instance, a function, a `bigint` or a value that contains itself. So a JavaScript host gets exactly the diagnostics a native host gets for the same values.
- A missing property and a property that is `undefined` are both absent. A payload that isn't given is absent too.
- `-0` crosses in as `-0`, and never comes out (§9.8.2).
- A render keeps its snapshot as it was encoded when `render` was called, so changing the host's objects afterwards changes nothing.
- A problem with the host's values is a diagnostic, never an exception. The promises reject only with a `TypeError` for arguments of the wrong JavaScript type, or a `render` the package didn't make; with `MeshVersionError` for a module of another version; and with `MeshInternalError` if the runtime itself fails.

## The render tree

The render tree is [`schemas/render-v1.schema.json`](../../schemas/render-v1.schema.json). The keys, handler identifiers and program identities in this manual's examples are illustrative, though their layout is the runtime's. It contains exactly primitive component names, prop names with values from the boundary data model (§9.8.1), the text of each number, boolean and `null` prop, text runs as strings, event names each with a handler identifier, and keys. It contains nothing else: no expression, scope name, command, argument, composite name, template, span or `$event`.

- **A node** is a primitive occurrence: `{ "type": "node", "key", "component", "props", "events", "children" }`, and `"propText"` when it has any.
  - `props` maps each written prop to its value: a semantic value, evaluated and checked against the prop's declared type, not realized for any target (§9.8.2). An absent value is an omitted prop; `null` is `null`.
  - `propText` (since v0.6) maps each prop whose value is a number, a boolean or `null` to that value's MESH text (§9.7.7): `"42"`, `"false"`, `"null"`. A string prop has no entry, since its text is its value; a list or record prop has none, since it has no text (§9.7.8); an absent prop has no entry in either map. The member is present exactly when some prop has an entry. A renderer that puts such a prop in a slot that holds only text uses this text (§9.8.7).
  - `events` maps each event binding's event name to its handler identifier.
  - `children` lists nodes and text runs, in order.
- **Composites never appear.** A composite occurrence is replaced by what its template produced: exactly one node, since a template has one root element.
- **A text run** is `{ "type": "text", "key", "text" }`: the text (§9.7.7) of a maximal sequence of adjacent literal text and interpolation children. A text run is present even when it's empty.

The example's components are the slice's (`examples/slice/components.json`), and every value in it fits that manifest: `avatar`'s `src` is a `string?` and its `size` a `string`, and `button`'s `disabled` a `boolean`, whose text is in `propText`.

A program with no conditional or repeated site is **static**: its render trees all have the same structure, whatever the snapshot, and only prop values and text differ. MPRX has no conditional or repeated syntax, so every program written in plain MPRX is static. The two provisional constructs below, `mesh-if` and `mesh-each`, are how a program's structure varies in this version, and the render tree can carry children that differ from one render to the next. What a key means when they do is the identity contract of spec §9.10, summarized under [Keys](#keys).

```json render-v1
{
  "format": "mesh-render",
  "version": 1,
  "root": {
    "type": "node", "key": "kD2_VHca9Ag2dUolhbZRbBg", "component": "page",
    "props": { "title": "Team" },
    "events": {},
    "children": [
      { "type": "node", "key": "k3oBD74HDIG2MzUDltfcVfw", "component": "avatar",
        "props": { "src": "ada.png", "alt": "Ada Lovelace", "size": "sm" },
        "events": { "click": "hDPuYKw885Yg.jn4fVuysv6d-WkiS01rl6A" },
        "children": [] },
      { "type": "node", "key": "kJ0mCg4AmY3XDu2vF9Hq1rw", "component": "button",
        "props": { "disabled": false },
        "propText": { "disabled": "false" },
        "events": { "click": "hDPuYKw885Yg.Tq3oZ8vN1WcI5yXeLr0bKg" },
        "children": [
          { "type": "text", "key": "kSPIXXSPi5uAsdGkh9bPbbA", "text": "Refresh" }
        ] }
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

A key is the render-v1 token for a node's or text run's **identity** (spec §9.10): two nodes in renders of one program have the same key if and only if they have the same identity. An identity is a sequence of *(site, instance)* steps from the root. A **site** is a place in the program's templates, named by template positions and components, not by a position in a render tree. An **instance** exists only at a *repeated* site, where it is a key the application declares. A program with no `mesh-if` or `mesh-each` has no conditional or repeated sites, so its identities are its sites alone, and the encoding below, without the provisional steps, is the whole of it. Keys are:
- **derived from the program:** for a static program, from the position, the sequence of child positions from the root template's root, with the component at each step, composites included, so a composite's expansion is part of every key below it. For a static program that position is the same place as the site (spec §9.10.10);
- **independent of values:** no snapshot or payload affects any key this MESH produces. The one value that enters an identity is a key an application *declares* for a repeated item (spec §9.10.3), which only a `mesh-each` can declare;
- **unique:** no two nodes or text runs in one tree share one;
- **stable within one program, for one identity:** every render of a program gives the same key to the same identity. For a static program that means every render has the same keys at the same positions. Once a program's structure can vary, the set of keys, and the position of a key, can differ from render to render, but a key still names the same node and is never a position. Keys are **not** stable across programs: changing the root or any template (spans included) may change every key. That is intentional, and there is no compatibility guarantee for keys across programs;
- **opaque:** a renderer may compare keys for equality, and must not interpret them;
- **not data identity,** nor application identity, except where an application declares a key for a repeated item (spec §9.10.3). That is the extension "a later design for lists" was reserved for: specified as a contract (spec §9.10), and implemented by the provisional `mesh-each` (below).

**Encoding of a static program's keys.** `H` is SHA-256; strings and counts are written as in the fingerprint (`docs/manual/templates.md`: a string is its 32-bit big-endian UTF-8 byte length and its bytes; a count is 32-bit big-endian). A **path** is the list of steps from the root. Each step is a position (a count), a kind byte, and a component name (a string):

- the root is step `(0, 0x01, its component)`;
- the child whose **site** is at position `i` among a node's template children (spec §9.10.2) is `(i, 0x01, its component)` for a node, or `(i, 0x02, "")` for a text run. A maximal run of text and interpolations is one site, an element is one, and a [conditional](#conditionals-provisional) is one whether or not it produces a node. Without a conditional that is the child's index in the render tree's `children`, which is why every static program's keys are as they were;
- a composite occurrence adds two steps: `(i, 0x03, the composite)`, then `(0, 0x01, the component of its template's root element)`, and further expansions nest the same way;
- a conditional alternative adds a step, `(i, 0x04, "consequent")` or `(i, 0x04, "alternate")`, where `i` is the conditional's site position, then the alternative's own steps at position `0`, as a composite's expansion does. This step is provisional (below).

The key is `k` followed by the first 16 bytes of `H(string "mesh-key-v1", the program identity (32 bytes), count of steps, each step)`, in unpadded base64url: 22 characters. This is the static case of identity. How an identity that has an instance is encoded is not decided (spec §9.10.10, §9.10.12).

**Why the program identity is in the hash.** The encoding is public. Without the program identity, anyone holding a key could hash guessed paths and component names, such as `user-card`, and confirm a guess, recovering a composite's name, which I13 rules out. With it, confirming a guess needs the program identity, which is a digest of the program's templates. A renderer is given neither: the handler identifiers it sees carry only the identity's first 8 bytes.

**What this does and doesn't provide.** It keeps structural names out of the render tree in a form that can be checked from the tree alone. It is **not** a cryptographic protection of the program:
- keys are unkeyed SHA-256 digests, truncated to 128 bits. They are neither secret nor authenticated, and they don't hide how many nodes there are, or the tree's shape;
- anyone who has the program's templates (for instance, a web page that ships them to the browser beside its renderer) can compute every key and handler identifier, and so match them to names;
- a host must not rely on keys or handler identifiers to keep templates, names or commands confidential, or to authenticate an event. Dispatch validates every handler identifier against the render it's given; that, not secrecy, is what stops a forged one.

**The cost:** keys change whenever the program changes. The tree doesn't say which program it came from, and a renderer mustn't try to infer it (handler identifiers are opaque). The host knows, because it made the render: when it gives the renderer a tree from a different program, it tells the renderer to draw it afresh rather than reconcile it by key.

The runtime checks every tree it builds for duplicate keys, and a collision is an internal error, `runtime-key-collision`. At 128 bits one doesn't happen in practice; the check makes "unique" a guarantee rather than a probability. That is a different failure from two nodes with the *same identity* (the same declared key at one repeated site): spec §9.10.4 makes that a diagnostic, fail closed, with no render tree. Its code is not decided.

### Conditionals (provisional)

*A tracer for spec §9.10, not a language feature.* MPRX has no conditional syntax, and nothing here decides one. To show that identity can keep the §9.10 rules when structure varies, the runtime gives one reserved component, `mesh-if`, conditional meaning. It is declared in the model like any component (with a required boolean `when`), so the parser, the checker and the compiler are unchanged: `<mesh-if when={show}>…</mesh-if>` is an ordinary element to them, and `when` is checked as any boolean prop is.

- **Alternatives.** Its one or two element children are its alternatives. The first is chosen when `when` is true, the second, if there is one, when it is false. With one child, a false `when` produces nothing.
- **It is never a node.** The render tree has what was chosen, or nothing, like a composite's expansion.
- **Each alternative is a distinct site,** even when two are the same component in the same place. The conditional is one slot among its siblings, so the siblings after it keep their positions, and so their keys, whether or not it produced a node. A node's identity does not depend on where it lands among the rendered children.
- **A program's handlers include its alternatives'.** Dispatch accepts an identifier only if its node is in the render it is given (§9.10.7); another alternative's identifier is `runtime-unknown-handler`.
- **A malformed conditional is refused** by program validation, with `assembly-malformed-template`: no `when`, events, anything but one or two element children, a conditional as an alternative, or a conditional as a template's root.

Not decided: the spelling, whether the compiler should check the shape, nested conditionals, and the encoding of the step above.

### Repeated structure (provisional)

*A second tracer for spec §9.10, not a language feature.* MPRX has no repetition syntax, and nothing here decides one. As `mesh-if` does for conditionals, the reserved component `mesh-each` shows that identity can keep the §9.10 rules when one site yields many nodes. It is declared in the model with three props: `items` (a list), `as` (a string) and `key`. `<mesh-each items={todos} as="todo" key={todo.id}><row on.tap={toggle(todo.id)}>{todo.title}</row></mesh-each>` is an ordinary element to the parser, and the checker and compiler give three things only: they type `items` as any list prop, they bind the name `as` writes (a literal) to the item's type, in `key` and in the child, and they check the key's type (below). The bound name exists only in `key` and the child; it is not a name of the component's scope, has no resolution an editor can follow, and the runtime puts it in a copy of the scope for each item.

- **One node per item,** in the order of the items. The element is never a node; an empty list produces nothing, and the slot is still one slot among its siblings, so the siblings after it keep their positions and keys.
- **A node's identity is its site and its declared key.** The step is the repeat's slot and the key, as a string or a finite number (kind `0x05`, the slot, and the key as `s:` and the string, or `n:` and the number's text). The item's index is never in it, so reordering, inserting before and removing others leave an item's key and handler identifiers alone. Two repeats are two sites: equal keys under them are different identities.
- **The key is evaluated at render, for each item, with the item bound.** Only the runtime has the value, so keys are a function of the program and the snapshot together, and are deterministic in them. It must be a string or a finite number; `"1"` and `1` are different keys, `1` and `1.0` the same, `0` and `-0` the same (a number's key is its text, §9.7.7.1, which has one text for both zeros).
- **The key's type is checked where the template is** (spec §9.10.3). A key of type `string`, `number` or `any` is accepted; a boolean, `null`, a list, a record, and a string or number that may be absent are refused with the ordinary `type-mismatch`: "a repeat's key must be a string or a number". `any` is accepted because every type check leaves `any` to the runtime. The rule is the checker's own, whatever the model declares for the `key` prop: the manifest's types have no union, so no declaration could say "string or number". Finiteness is a value's, so the runtime checks every key it evaluates, and `runtime-invalid-key` stays for the values types can't exclude (an `any` holding `null`, say). An absent key can't reach it from a program that compiles; the check is a backstop.
- **A bad key fails closed.** A key that is absent, `null`, a boolean, a list or a record is `runtime-invalid-key`; two items of one repeat with the same key are `runtime-duplicate-key`. Nothing is rendered: the runtime never substitutes a position, and never picks one of two items.
- **An item that leaves and returns is a new occurrence.** `[A]`, then `[]`, then `[A]` give A the same identity in the first and third renders and **create** it in the third: the identity is a name, not an object, and nothing a target realized for the first survives the absence (spec §9.10.6).
- **Dispatch needs the snapshot to find a repeated node.** The requirement (spec §9.10.7): a repeated handler's arguments depend on its item, an identifier is a hash and can't be turned back into one, so the item's scope is derived from **the program and the current snapshot**, never from a rendered position, a target or the application's objects. The mechanism today: for a program with a repeat, dispatch renders from the render's program, model and snapshot (all of which a render keeps), records each handler with the scope it had, which has the item, and evaluates the handler's arguments there. The mechanism is internal and may change; the requirement may not. An identifier whose item isn't in the snapshot is `runtime-unknown-handler`; if the snapshot can't be rendered (a duplicate key, say) dispatch reports why. Without a repeat, dispatch needs no values to find a handler, as before.
- **A malformed repeat is refused** by program validation, with `assembly-malformed-template`: no `items`, `key` or non-empty literal `as`, events, anything but exactly one element child, a conditional or another repeat as that child, or a repeat as a template's root. Nested dynamic structures are not part of the tracer.
- **render-v1 is unchanged.** A repeated node is a node with a key; the order of its parent's `children` is the order of the items.
- **The encoding is provisional.** The `0x05` step and the `s:`/`n:` spelling of a key inside it are an implementation detail, not the contract. The contract is that an instance's identity is its declared string or finite-number key, under the equality above.

`examples/conformance/identity/repeat/` has the language-neutral vectors for these rules (C11–C22), in render-v1 and with no syntax; `crates/mesh-runtime/tests/repeat.rs` renders their keys and checks the runtime agrees.

Not decided: the spelling, nested repeats, the encoding of the step above, and what a renderer does with an item that leaves and returns.

### Identity, realization and lifetime

A key names a node. It is not the target's object for that node. Three things have three owners (spec §9.10.6):

- **Semantic identity** is MESH's: which node of the program's output an occurrence is. It is what a key carries.
- **Realization** is the PORT's and its target's: the object or objects held for the node. Nothing in a key says whether one exists, or what it is.
- **Lifetime** is the run of consecutive renders in which an identity is present. A node that is absent from a render is not realized, and its occurrence has ended. If its identity is present again later, that is a new occurrence: **created**, inheriting nothing. The same identity returning is the same *name*, not a surviving realization. Whether a target may cache anything across the absence is not decided.

So between two renders of one program a renderer classifies each node as **kept** (its key is in both trees), **created** (only in the later) or **removed** (only in the earlier), and a kept node as **moved** when its order relative to another kept sibling is reversed. `examples/conformance/identity/` pins these outcomes, and shows where matching by position would get them wrong.

### Program identity

A program's **identity** is `H(string "mesh-program-v1", string root, count of templates, then for each template, in code-point order of its component: string component, its canonical digest (32 bytes))`. A template's canonical digest leaves out `compiler` (`docs/manual/templates.md`). The identity changes whenever the root or any template changes, and is never given to a renderer.

### Handler identifiers

A handler identifier names exactly one handler at one node. It is determined by the program's identity, the node's key (its identity) and the event's name. So it is unique within a program, the same in every render in which its node is present, and independent of values. When a program can have repeated nodes (spec §9.10.7), two occurrences of one template handler at different instances will have different identifiers, a node that moves among its siblings keeps its handler identifiers, and an identifier changes only when its node's identity does. It is not the command, and it contains nothing from which the command or its arguments can be recovered without the program's templates. The same limits apply as for keys: it is opaque, not secret, and dispatch's validation, not its unpredictability, is what makes it safe to accept from a renderer.

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

## The declared events

Besides render and dispatch, the runtime can say which events a **program** declares. A **declared event** is one event binding (`on.click={open(item.id)}`) in one of the program's templates: a fact about the validated program, not about a render. It is there whether or not any render contains it.

Each declared event is:

- **`component`:** the component whose template declares the binding. It is the component of the command's intent (see [The command intent](#the-command-intent)): the declared event `list`/`open` is the event whose intent is `open` of `list`.
- **`event`:** the event of the element the binding is on (`click`).
- **`command`:** the command of `component` that the event raises (`open`).
- **`span`:** the binding's span in the source of `component`'s template, as the template records it (UTF-8 bytes and UTF-16 code units, from the start of the source). It carries no file name: the source of a template is the host's.

What is declared:

- **Every template of the program,** composites included. A composite's template is walked once, however many times it is used, so `user-card`'s `selectUser` is declared once.
- **Every element of each template,** so an event inside a `mesh-if` alternative the snapshot doesn't choose is declared, and an event inside a `mesh-each` body is declared once, whatever the items. A rendered handler belongs to a node, and a repeated node's identity needs the item's values; a declared event needs neither, so it has no handler identifier.
- **Nothing is deduplicated.** Two bindings that raise the same command are two declared events, with their own spans.
- **A component with no template is a primitive** ([the program decides](./templates.md#programs), never the template), so the commands the manifest declares for it are not declared events: a program that doesn't supply `user-card`'s template renders `user-card` as a primitive that raises none.

They are in a fixed order: the templates' components in order of name, then each template's bindings in document order, whatever order the host supplied the templates in.

A program that isn't valid has no declared events. The operation validates the program exactly as render and dispatch do (and as `mesh check-program` does), and gives that program's diagnostics.

**From Rust,** `mesh_runtime::declared_events(&program, model)` returns `Ok(Vec<DeclaredEvent>)` or `Err(diagnostics)`, the diagnostics `mesh_runtime::check_program` gives. `mesh_runtime::declared_events_to_json` writes them as the array below.

**From JavaScript,** `declaredEvents({ program: { root, templates }, model })` resolves to `{ events }`, an array of objects of the form below, or `{ diagnostics }`. It needs no snapshot. The package transports the result and judges nothing.

The JSON form of one declared event (the WebAssembly module returns `{"events": [ ... ]}`, or `{"diagnostics": ...}` as render does):

```json
{
  "component": "list",
  "event": "click",
  "command": "open",
  "span": {
    "start": { "byte": 162, "utf16": 162 },
    "end": { "byte": 183, "utf16": 183 }
  }
}
```

## Event resolution

Which binding one interaction reaches is MESH's rule (§9.9), the same on every target, and needs only the render tree:

1. Start at the **interacted node**: the innermost node the interaction is on (an interaction on a text run is on its parent node).
2. If that node's primitive has an **applicable event** for the interaction (the one of its events the interaction constitutes, by the PORT's mapping of its target's interactions onto the primitive's MESH event contract), and the node binds that event, that binding receives the interaction. Stop.
3. Otherwise go to the parent, towards the root. If no node qualifies, nothing is reported.

So one interaction gives at most one handler identifier, one dispatch and one intent. This is **event resolution, not DOM bubbling**: there are no phases, no event reaches a second binding, and MPRX has no syntax to change any of it.

A renderer implements the rule for its target, and checks itself against `examples/conformance/events/`: real programs and trees, interactions (a node key, and the applicable event per primitive), and the handler identifier and intent each resolves to, or none. From Rust, `mesh_runtime::resolve(&tree, &Interaction { target, applicable })` is the reference implementation: it returns the `Resolved` binding (its node's key, the event and the handler identifier) or `None`, and refuses a target key that isn't in the tree. It evaluates nothing and needs no program, so a renderer needs none of MESH's code to do the same.

## Updates

- **A change of values is a new render,** or an *update*, which gives the same tree and says what changed. `render` evaluates the whole program again and gives a complete new tree; `update` (below) does that for a render's program and a new snapshot, and also returns the patches from the previous tree to the new one.
- **Every render of a program names its nodes by identity, and a key is that identity.** For a static program every render has the same structure, keys and handler identifiers, so a renderer can match a new tree against the one it drew, node by node, by key, and update in place. For a program whose structure varies (a `mesh-if` or `mesh-each`, spec §9.10) the rule is the same for a tree that differs: match **by key, not by position**. A key in both trees is the same node, kept and moved if its order among its siblings changed; a key only in the new tree is created; a key only in the old one is removed. A tree from a **different** program (a template changed or was added) may have entirely different keys: a renderer draws it afresh, and never matches it against the old tree by key.
- **Finding what changed is the runtime's job when the host asks.** `update` (below) renders a new snapshot of the same program and returns the patches from the previous tree to the new one. `render` alone still gives a complete tree, and a renderer that compares trees by key itself loses nothing.

### Update

`update(previous, snapshot)` (Rust: `mesh_runtime::update`; JavaScript: `update(render, snapshot)`) takes a render and a new snapshot of **the same program**, and returns the new render and a `render-patch-v1` list ([`schemas/render-patch-v1.schema.json`](../../schemas/render-patch-v1.schema.json)), or diagnostics. The host, as for a renderer's `update`, asserts program continuity: a different program is a `render`.

- **The law.** Applying the patches, in order, to the previous render's tree gives exactly the tree a full `render` of the new snapshot gives. A list is minimal in effect, not unique in form, so a renderer applies lists and never compares them. The runtime's tests check the law over random snapshots of a program with a conditional, a keyed repeat and an optional prop, and check every list against the schema.
- **The operations.** Parts are named by their render-v1 keys, and matched by key, never by position (spec §9.10).
  - `setProp` (with `propText` when the value has text), `removeProp` and `setText` change a kept node or text run.
  - `insert` adds a node (with its whole subtree) or text run under a parent, before a sibling or last; `remove` takes a part and everything under it away; `move` puts a kept part before a sibling, or last. A kept part is kept: a renderer keeps its realization across a `move`, and a key that is removed and later appears again is a new part.
  - **The order of a list is the order to apply it in:** removals first, then the insertions and moves that bring the kept parts into the new order (each `before` names a sibling that exists at that point), then the changes inside kept parts. A list that reorders needs only moves and inserts, never a rebuild: moving one item of a long list to the front is one `move`.
  - `replace` is for a tree no other operation can turn into the next (a different root, or a node whose event bindings differ, which one program never produces). It is always the only operation of its list, and a renderer draws it afresh, reusing nothing.
- **What it skips.** A render's tree is shared, not copied, and each node remembers what it was computed from: the values, in its scope, of the scope names its subtree reads from outside it (a template says them statically: a name an expression mentions, less the name a repeat binds inside), of the names its own props read, and, for each text run, the values its text was made from. On `update`:
  - **A node whose subtree's inputs are all identical is the previous node itself,** and its subtree is not looked at. A number is compared by its bits (so `0` and `-0` differ). If nothing the root reads has changed, the new render's tree *is* the previous tree and not one expression is evaluated.
  - **A repeat remembers its items.** An item that is identical to the one at its place before, with what its node reads as it was, is reused without evaluating its key, building its path or hashing it. An item that moved, or came in, is rendered as before.
  - **A node on a changed path is rebuilt from its parts:** its props are reused if the names they read are as they were, and so is each text run.
  - **The patches skip the shared parts too:** the diff does not look inside a node that is the very same node in both trees, and a list of children in the same order needs no lookup tables.
  - **Still done on every update:** validating the snapshot (the work is the snapshot's size, and is what remains when nothing has changed), `mesh-if`'s `when`, `mesh-each`'s `items`, and a visit to each node on a changed path, and to each item of a repeat on one.
- **A refused update changes nothing.** On diagnostics, `previous` is untouched and still dispatches.
- **The handler identifiers are unchanged by an update,** since they come from identity (§9.10), so a drawn tree's handlers remain valid for the new render, and a part inserted by a patch has the handler identifiers a full render would give it.

**Retained renders in the module.** In Rust a render is a value, and `update` takes the previous one by reference. The WebAssembly module, which a JavaScript host calls, keeps the renders `update` returns, so that the next `update` from one has what it needs and doesn't derive it again:

- **`render()` keeps nothing in the module.** A render it returns has no copy there, so the first `update` from it derives the render again from its own inputs (a render more than the same update costs in Rust), and the render that `update` returns is kept.
- **`update` keeps the render it returns, under a handle the module never reuses,** and keeps `previous` too, until released: after an update the host may still dispatch with `previous` (events fired before its renderer applied the patches), or update from it again.
- **`release()` on a render ends the module's copy of it.** A host calls it on a render it will no longer update from or dispatch with, typically `previous` once the patches are applied. It is safe to call twice, and a released render still works: it keeps its own inputs, so a later `update` derives it again. A render that is never released is released when it is garbage collected, but that is up to the JavaScript engine and can be late, and until then its copy is memory in the module. The package's memory test releases every render in a chain of ten thousand updates and checks that memory doesn't grow.
- **A copy is good only in the module that made it.** The package discards its module after a trap and uses a new one, which has none of the old one's renders: a render whose copy is gone updates by deriving itself again, with the same result.
- **`dispatch` still evaluates against the render's own snapshot,** as before, and holds nothing in the module after the call.
- **The module returns the patches, not the new tree.** The caller already has the previous tree, and applying the patches to it gives the new one (the law above), so what crosses the boundary on an update is what changed. The JavaScript package applies them (`src/patches.ts`) as a renderer would, over immutable data: the new tree is frozen and shares with the previous tree every part the patches didn't reach, so a part that didn't change is the very same object. The package's review test names that file as the one place it reads a tree, and checks that it makes no text and no number.
- **The module's calls are one at a time,** so retention adds no ordering of its own: a `release` is queued behind the calls already made.

Measured on one machine, a keyed list of 10,000 items with one changed (the least of 15 runs; `cargo test -p mesh-runtime --release --test update_baseline full_render -- --ignored --nocapture` in Rust, `node scripts/bench-update.mjs` in the package):

| | full render | update, one item changed | update, nothing changed |
|---|---|---|---|
| Rust: expressions evaluated | 60,001 | 7 | 0 |
| Rust: time | 91 ms | 21 ms | 12 ms |
| JavaScript: time, across the module's boundary | 481 ms | 77 ms | |

At 1,000 items the update is 1.6 ms in Rust against 6.2 ms to render, and 6.2 ms in JavaScript against 47 ms. What remains is the size of the snapshot, not of the tree: the snapshot is encoded, decoded and validated on every update (12 ms of the 21 in Rust, and the largest part of JavaScript's), and the per-item comparison in a changed repeat. An update that costs only what changed would have the host hand over the changes to the snapshot, not the snapshot: that is designed (the spec's A6) and not built.

## What renderers and hosts must do

A **renderer**:
- realizes each node's props, and each text run's text, **as given** (§9.8.7): a prop value natively, in a target slot that holds a value of its kind exactly, or, in a slot that holds only text, as its MESH text: a string prop's value, or the prop's `propText` entry. It computes nothing: it doesn't format numbers, convert or coerce values (by a platform's conversion, a host language's formatting, JSON or anything else), or supply a missing prop. A value with no conforming slot, such as a list or record where only text fits, is the renderer's failure to report, never a value to substitute;
- resolves each interaction to at most one binding by event resolution (§9.9), and reports that binding's handler identifier and the event's payload, never interpreting either;
- compares keys only for equality, and reconciles by key, never by position, and only between trees its host says come from one program;
- surfaces a node of a component it doesn't know, rather than dropping it (a composite whose template a program left out arrives as a node of its name).

A **host**:
- keeps each render while its tree is on screen, and dispatches each event with the render whose tree the renderer had drawn;
- tells its renderer when a new tree comes from a different program than the one drawn, so the renderer draws it afresh instead of reconciling by key;
- shapes its values to the manifest: records are exact, so a record with more fields than its type declares is refused (§9.8.4);
- maps intents to its own commands, and treats diagnostics as errors in its inputs or its program, not as messages for end users.

## Number to text

Text for a number is §9.7.7.1's, in text runs and in `propText` alike, and nothing else decides it: the shortest digits that read back as the number, the nearest of those, and the even one of two equally near. The runtime generates these digits with its own exact code (`mesh_runtime::number_to_text`), and reproduces every row of the normative table, [`docs/tables/number-to-text.tsv`](../tables/number-to-text.tsv), natively and in WebAssembly.
- **It doesn't use Rust's standard formatting,** which rounds the equally near cases up: on Rust 1.98.1 it fails 4 of the table's 82 finite rows, all such ties. Its layout differs too: it never uses an exponent.
- **It doesn't use `ryu`** in v0.5, because there is no citable evidence that it chooses the nearest candidate and breaks ties to even. A library could replace MESH's code only with that evidence.
- **It isn't a JavaScript engine's `String(x)`.** ECMA-262 leaves the last digit open, so an engine may differ from MESH without breaking ECMA-262. A differential test compares MESH with V8, and found no difference over 1,000,000 values on Node 22, but V8 is not the authority. A renderer draws MESH's text as given (a text run, or a prop's `propText`), and never formats a number itself.

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
