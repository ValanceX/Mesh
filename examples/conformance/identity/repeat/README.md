# Repeated-site identity vectors (spec §9.10, C11–C22)

Contract vectors for **repeated** structure: what makes an item in one render the same node as an item in the next. They extend [`../`](../README.md), whose definitions of `previous`, `next`, `earlier`, `identities` and `expect` (kept, created, removed, moved, handlers, positional) they use unchanged.

Like those, they are **render-v1 and nothing else**. They contain no MPRX, and nothing in them depends on how a repeat is spelled: the provisional `<mesh-each>` of the runtime manual does not appear. Every tree validates against [`render-v1.schema.json`](../../../../schemas/render-v1.schema.json).

## What is new

- **`keys`** (in `previous`, `next`, `earlier`): the **declared key** of each instance of each repeated site, in item order, as `{ "site": [keys…] }`. A key is a string or a finite number, and these are what the tree's identities are written from.
- **Identity names** are `page/<site>/item[<key>]`, a string key **quoted** (`item["1"]`) and a number **bare** (`item[1]`), and `0` and `-0` both written `0`. The notation is for these files only; it is not an encoding.
- The tokens in `key` and handler positions are illustrative, as in `../`; they are derived from the identity, so equal identities have equal tokens throughout the file.

`crates/mesh-compiler/tests/identity_vectors.rs` checks the vectors against their own definitions: each identity is its site and declared key, the declared keys of a site are pairwise unequal under §9.10.3, the rendered order is the keys' order, and every expectation follows from the identities. `crates/mesh-runtime/tests/repeat.rs` renders the declared keys through the runtime's provisional repeat and checks that it classifies and refuses as the vectors say.

## The vectors

| Vector | Pins |
|---|---|
| `C11-stable-repeated` | the same keys, same order: all kept, none moved |
| `C12-reorder` | `[a, b, c]` → `[c, a, b]`: all kept, all moved; position pairs the wrong nodes |
| `C13-insert` | `[a, b]` → `[a, x, b]`: `x` created, `a` and `b` kept |
| `C14-remove-middle` | `[a, b, c]` → `[a, c]`: `b` removed |
| `C15-remove-first` | `[a, b, c]` → `[b, c]`: `a` removed, though every index changed |
| `C16-duplicate-key` | `[a, b, b]`: fails closed, **no tree** |
| `C17-invalid-key` | `null`, a boolean, a list, a record, an absent key: each fails closed, **no tree** |
| `C18-string-versus-number` | `"1"` and `1` are different identities; removing one keeps the other |
| `C19-zero-normalization` | `0` → `-0`: one identity, kept |
| `C20-same-key-different-sites` | `"a"` at two repeated sites: two identities, no collision |
| `C21-reappearance` | `[a]` → `[]` → `[a]`: same identity, **created → removed → created**; `lifetime` records the realization's |
| `C22-handler-identity` | a repeated node's handlers are its identity and event: stable across a reorder, distinct per key and per event |

`C16` and `C17` have no trees: a render-v1 tree has unique keys and valid ones, and a render with a duplicate or invalid declared key is rejected before there is one. `NaN` and the infinities are not keys either, but JSON cannot write them, so there is no entry. `C17`'s absent key is written `{ "$absent": true }` and stands for a key with no value.

**Identity is not realization.** `C21` is the case that needs saying twice. The identity of `a` is the same in the first and third trees because it is a name. That does not make the object a target built for the first tree come back: it was disposed when `a` left, and the third tree creates a new one.

## What these do not decide

The syntax for a repeat and for declaring a key; how an identity is encoded into a key (the runtime's `s:`/`n:` is an implementation detail, not this contract); nested repeats; how a renderer reconciles; how hydration transports identity; and how dispatch recovers an item's scope, beyond the requirement that it come from the program and the snapshot. See [§9.10.12](../../../../docs/MPRX-SPEC.md).
