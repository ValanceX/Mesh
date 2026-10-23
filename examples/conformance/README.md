# Conformance vectors

Language-neutral cases for MESH's semantics at the render tree, for anyone who builds a renderer (a PORT) and for MESH's own tests. They need no target, no browser and none of MESH's code: every file is JSON or MPRX.

Each directory except [`identity/`](./identity/README.md) is one real program, against [`components.json`](./components.json):

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

## `enums/`: enum types, `has`, `??` and prop defaults (spec §9.2, §9.3, §9.4, §9.8)

This program has **its own `components.json`** (the shared one, and so every other program's keys, is untouched). `cases.json` is `{ props, rejects, dispatches }`:

- `props`: a node's prop, present (`value`) or `absent`: an enum is carried as its string, `??` takes its default, and a composite's unwritten prop binds its `default`;
- `rejects`: a snapshot that doesn't render, and the first diagnostic's `code` and `location` (a string that isn't a variant is `runtime-value-mismatch` at its path, through `any` to an enum prop `runtime-prop-mismatch`);
- `dispatches`: an event reported on a node with a payload, and the `intent` it gives, or the first diagnostic's `code` and `location` (`runtime-value-mismatch` at `$event`; `runtime-argument-mismatch` at the argument).

A renderer needs none of this: an enum is a string in the tree, and the tree's format is unchanged. A host runtime implementing MESH's boundary does.

## `snippets/`: typed template values (spec §9.2 to §9.4, §9.10.2; decisions U1 to U14)

This program has **its own `components.json`** too, and is a `template-v2` program (`mesh-snippet`, `mesh-render`). It renders a generic list whose rows the caller supplies (`data-list`), the same through a second composite that passes the snippet along (`shell`), a card that takes `children` and an optional `footer`, local snippets rendered twice, a snippet that renders itself over a tree, and a snippet declared inside a repeat's item. Beyond `snapshot.json` and `expected.tree.json` it has **steps**: `step-N.snapshot.json` with `step-N.expected.tree.json`, a reordered list, an edit that changes a card whose own props do not change, and an emptied one. Each step renders its tree, and updating from a step to the next is the render of the next (the memo hazard of slots, for snippets).

`cases.json` is `{ distinct, same, different }`, relations between keys. A node is named by `step` and `path`, the indexes of the nodes (text runs not counted) from the root down: `distinct` lists nodes of one step whose keys are pairwise different; `same` pairs two nodes, possibly of different steps, with the same key (a row follows its item when the list is reordered); `different` pairs two with different keys.

## `events/`: event resolution (spec §9.9)

Each case gives an **interaction** and what it resolves to:

- `interaction.target`: the key of the innermost node, or text run, the interaction is on;
- `interaction.applicable`: for each primitive component, the one event of it that the interaction constitutes: already determined, as a PORT's mapping of its target's interactions onto the primitive's MESH event contract would give it. A component with no entry has none;
- `expect`: `{ "node", "event", "handler", "intent" }`, the binding that receives the interaction and the intent its handler dispatches to (with no payload), or `{ "none": true }`.

Resolution examines the target node, then each ancestor in turn, and stops at the first node whose component has an applicable event and which binds it. So every case resolves to at most one binding and one intent. The cases cover a child's binding winning over its ancestors', an ancestor receiving the interaction when the child has no applicable binding, text runs, nodes with no events, several ancestors with bindings, unrelated bindings and event names shared across components, a composite boundary, and an interaction that resolves to nothing.

A renderer implements the rule against its own target (hit testing, then the walk) and checks, for each case, that it reports exactly the expected handler identifier, or nothing.

## `identity/`: node identity (spec §9.10)

Unlike the directories above, this one is **not** a real program, though MPRX does express conditionals and repeats (`{#if}` and `{#each}`, since v0.11). The vectors are written as render-v1 trees by hand, so they pin the rule without depending on a compiler or runtime, and no MESH produced them. `identity/cases.json` holds hand-written before/after render-v1 trees, the identity of every node in each, and what the contract says happened between them: kept, created, removed, moved, and which handler identifiers stay the same. It also gives the pairing that matching by position would make, so a renderer can see exactly where that is wrong. It needs no MPRX, no manifest and no compiler.

See [`identity/README.md`](./identity/README.md) for the format and the meaning of each outcome.

## `forwarder/`: a composite that is only `{@render}` (decision U13)

This program has **its own `components.json`** and is a `template-v2` program. `pass` is a template of nothing but `{@render children()}`: it makes no node, and what its caller wrote is placed in place (text, one node, several nodes, a row with a handler inside a repeat, either branch of a conditional). `relay` is a forwarder of a forwarder (`<pass>{@render children()}</pass>`), `labelled` a fragment composite that writes a node and then its `children`. Like `snippets/` it has steps (`step-N.snapshot.json` with `step-N.expected.tree.json`: a reordered list and the other branch, then one more item), and `cases.json` pins the keys: every forwarded node has its own, a row keeps its key through the forwarder when the list is reordered, and the two branches of a conditional are two nodes. A renderer needs nothing new for it.

## `passing/`: local snippets passed along as prop values, and optional snippets (decision U19)

This program has **its own `components.json`** and is a `template-v2` program. A local snippet is declared among the children of `page`, and then is the whole value of an attribute of an occurrence in them: `row={fmt}` to a `data-list` (the same `fmt` to two of them, at two positions), `row={note}` through `shell`, which passes it on again as `row={row}`; a local snippet declared inside a repeat's item and reading the item, given to `panel` as its optional `header`; `panel`'s optional `header` and optional `children` left out, which draw nothing (`panel` renders each only inside `{#if has(…)}`). The snippet is rendered in the scope it was declared in, not the holder's: the item and the title it reads are the declaring template's. Steps reorder the list, drop a flag and empty it; `cases.json` pins the keys (the same snippet passed twice makes two nodes; a row keeps its key when its item moves).

## `units/`: a component's state for one occurrence (docs/manual/runtime.md, "Units")

This program has its own `components.json`: `disclosure` has a unit (`open`, and `toggle`), `tabs` has one whose field `at` is `controllable`, and the page writes `at={tab}` on its tabs. `steps.json` is a list of `{ case, snapshot, units }`; `units` is `[ { instance, state } ]`, the state a host holds for the occurrences it has changed, by the key the render gave each (`step-N.expected.instances.json` lists the occurrences a render has, with the keys). Each step has a committed tree (`step-N.expected.tree.json`) and a committed list of occurrences (`step-N.expected.instances.json`), and updating from a step to the next, with the next's `units`, gives exactly the render of the next.

What the steps pin: with no state held an occurrence has the defaults; the state held for an occurrence is what its template sees and only its; a re-sorted list keeps the state with the row (the occurrence's key is its place by the item's declared key, not its position); a row that leaves is not listed and the state held for it is not used; and a `controllable` field the caller writes a prop for is the caller's value whatever the host holds.
