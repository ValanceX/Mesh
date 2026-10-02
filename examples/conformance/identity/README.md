# Identity vectors (spec §9.10)

Contract vectors for **node identity**: what makes a node in one render the same node as one in the next, when a program's structure can vary. They pin the rule in [spec §9.10](../../../docs/MPRX-SPEC.md) before any syntax, compiler or renderer implements it.

These are unlike [`values/`](../values/) and [`events/`](../events/). Those are real programs, rendered by the runtime. **These are not.** MPRX cannot yet express a conditional or repeated element, so no MESH produces these trees. Every tree is hand-written render-v1 (it validates against [`render-v1.schema.json`](../../../schemas/render-v1.schema.json)), and nothing here needs MPRX, a manifest or the compiler. They stay valid however a renderer is written, and whatever identity ends up being encoded as.

`cases.json` is the one file. The repository checks it is valid and consistent (`crates/mesh-compiler/tests/identity_vectors.rs`), which tests the *vectors*, not any renderer.

## A vector

| Field | Meaning |
|---|---|
| `name`, `section`, `about` | the case, the spec section it pins, and what it shows |
| `previous`, `next` | `{ "tree", "identities" }`: a render-v1 tree, and the identity of **every** node and text run in it, by key |
| `earlier` | only where a vector is about a node returning: the render before `previous` |
| `expect` | what the contract says happened between `previous` and `next` |

**Identity names** are a notation for these files only: `page/list/item[a]` is the node produced by the site `item` under `list` under `page`, with the declared key `a`; `/#text` is a node's text run. A conditional or static site has no `[…]`. It is **not** an encoding, and says nothing about how a runtime would write an identity.

**Keys and handler identifiers** in the trees are illustrative tokens: they match the schema's patterns, are readable on purpose, and are the same token for the same identity (and the same handler token for the same identity and event) throughout the file. They are not what any runtime would produce, and a renderer must still treat them as opaque, equal or not.

## What `expect` says

Lists name **nodes** (text runs follow their node: kept with a kept node, created with a created node, removed with a removed one).

- **`kept`**: the identity is in both trees. The target object that realized it for `previous` is the one that realizes it for `next`.
- **`created`**: only in `next`. **`removed`**: only in `previous`; its realization is disposed, and its handlers are gone.
- **`moved`**: a kept node whose order relative to at least one other *kept sibling* is **reversed**. A kept node whose index merely changed, because a sibling was created or removed, is **not** moved. A moved node is also in `kept`.
- **`handlers`**: `same` (identity and event in both trees, with the same handler identifier), `new`, `gone`, and `distinctWithinEachTree` (no two handlers in one tree share an identifier). `sameAsEarlier` appears only with `earlier`.
- **`positional`**: the pairing a renderer would make by matching the children of `parent` **by position**, as `[previous, next]` identity pairs, and whether that pairing agrees with identity (`correct`). It is a negative reference, not an algorithm anyone must follow: where `correct` is `false`, matching by position joins nodes that are not the same node. It is `false` for every vector except `C1`.

So the vectors make the contract observable. For identities in both trees the same target object is kept; for a new identity one is created; for a missing one it is disposed; and for the same identities at different positions the objects are moved, not recreated.

## The vectors

| Vector | Pins |
|---|---|
| `C1-stable-siblings` | nothing changes: all kept, none moved |
| `C2-insert` | `[A, B]` → `[A, X, B]`: A kept, X created, B kept |
| `C3-remove-middle` | `[A, B, C]` → `[A, C]`: B removed; A and C kept |
| `C4-remove-first` | `[A, B, C]` → `[B, C]`: A removed; B and C kept |
| `C5-reorder` | `[A, B, C]` → `[C, A, B]`: all kept, all moved |
| `C6-duplicate-key` | two occurrences of one identity: the render fails closed, and **no tree exists** (`wouldBe` lists the occurrences) |
| `C7-conditional-disappears` | `[A, X, B]` → `[A, B]`: X removed; its stable siblings kept |
| `C8-conditional-reappears` | `[A, B]` → `[A, X, B]` after X was present earlier: X **created**, a new occurrence that inherits nothing |
| `C9-handler-identity` | two items' click handlers differ, and follow their items through a reorder |
| `C10-same-label-different-identity` | two items that show the same text are different nodes: removing one keeps the other |

`C6` has no trees because a render-v1 tree has unique keys; a render with a duplicate identity is rejected before there is one. The diagnostic's code is not decided.

Repeated sites have their own vectors, C11–C22, in [`repeat/`](./repeat/README.md).

## What these do not decide

The syntax for conditional and repeated elements and for declaring a key; how an identity is encoded into a key; collisions; other key types; render-v1 changes; how a renderer reconciles; how hydration transports identity; how dispatch finds an item from an identifier; and whether a target may cache a realization across an absence. See [§9.10.12](../../../docs/MPRX-SPEC.md).
