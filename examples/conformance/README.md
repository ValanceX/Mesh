# Conformance vectors

Language-neutral cases for MESH's semantics at the render tree, for anyone who builds a renderer (a PORT) and for MESH's own tests. They need no target, no browser and none of MESH's code: every file is JSON or MPRX.

Each directory is one real program, against [`components.json`](./components.json):

| File | What it is |
|---|---|
| `program.json` | the program's root and its templates, in order |
| `<component>.mprx` | each template's source |
| `snapshot.json` | the root template's scope values |
| `expected.tree.json` | the render tree the runtime gives, exactly as it writes it (compact JSON) |
| `cases.json` | named cases about that tree |

The committed trees are real: `crates/mesh-runtime/tests/conformance.rs` renders each program and compares it byte for byte, and `packages/mesh-runtime/test/conformance.test.mjs` does the same through `@valancex/mesh-runtime`, value for value. A renderer can use the trees directly, without rendering anything.

## `values/`: a value, and its text (spec §9.7.7, §9.7.8, §9.8.2, §9.8.7)

Each case names a node (by key) and a prop, and gives:

- `value`: `{ "value": v }`, or `{ "absent": true }` when the node has no such prop;
- `propText`: `{ "text": t }`, or `{ "absent": true }` when the node's `propText` has no entry for it.

They cover strings (no entry: the value is its text), numbers (MESH's text, including the exponent layout, the smallest subnormal, an equally-near tie that only §9.7.7.1's rule decides, and `-0`), booleans, `null` (text `null`), an absent prop (neither entry, unlike `null`), and lists and records, which never have text, directly or through `any`. One case gives a text run, showing content uses the same text.

One case, `states`, pins the three states a prop can be in, each as a whole node: `exactly` is the node's `props`, and its `propText` member only when the tree has one.

| State | `exactly` |
|---|---|
| absent | `{ "props": {} }`: no prop, and no `propText` member at all |
| `null` | `{ "props": { "value": null }, "propText": { "value": "null" } }` |
| string | `{ "props": { "value": "hello" } }`: its text is its value, so no `propText` copy |

Each node must be exactly as given, and no two states may be written alike: absent ≠ `null`, `null` has the text `null`, and a string needs no copy.

A renderer realizing any of these props in a slot that holds only text must produce exactly the value (for a string) or the `propText` entry, and must refuse a list or record there. It never makes a text itself.

## `events/`: event resolution (spec §9.9)

Each case gives an **interaction** and what it resolves to:

- `interaction.target`: the key of the innermost node, or text run, the interaction is on;
- `interaction.applicable`: for each primitive component, the one event of it that the interaction constitutes: already determined, as a PORT's mapping of its target's interactions onto the primitive's MESH event contract would give it. A component with no entry has none;
- `expect`: `{ "node", "event", "handler", "intent" }`, the binding that receives the interaction and the intent its handler dispatches to (with no payload), or `{ "none": true }`.

Resolution examines the target node, then each ancestor in turn, and stops at the first node whose component has an applicable event and which binds it. So every case resolves to at most one binding and one intent. The cases cover a child's binding winning over its ancestors', an ancestor receiving the interaction when the child has no applicable binding, text runs, nodes with no events, several ancestors with bindings, unrelated bindings and event names shared across components, a composite boundary, and an interaction that resolves to nothing.

A renderer implements the rule against its own target (hit testing, then the walk) and checks, for each case, that it reports exactly the expected handler identifier, or nothing.
