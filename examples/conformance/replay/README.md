# `replay/`: session logs

A **session log** is a recorded session of a MESH program, small enough to read and exact enough to replay: the inputs the host gave the runtime and the hashes of the trees it got. Any MESH runtime must replay the logs here and give the recorded hashes (Bet 15, R1); a renderer (a PORT) is not asked to replay trees.

The checks are `crates/mesh-runtime/tests/replay.rs` (native) and `packages/mesh-runtime/test/replay.test.mjs` (WebAssembly). Both play every log by **three routes** and require the same hashes from each:

| Route | Does |
|---|---|
| `changes` | `render` the header's snapshot, then `update_changes` with each `changes` line (`base` is the player's own: render versions are process-local and never recorded) |
| `update` | the same, applying each `changes` line to a plain snapshot on the host and calling `update` with the whole snapshot |
| `render` | never `update`: `render` of each snapshot afresh (time travel) |

## The format (`format` 1)

Line-JSON: one object per line, `line` first. Appendable, and readable after a crash.

| `line` | Members | Meaning |
|---|---|---|
| `header` | `format`, `runtime` (the version that recorded it, informational), `program` (SHA-256 of the template sources in `program.json`'s order, each followed by a NUL byte), `manifest` (SHA-256 of the directory's own `components.json`, else of `../components.json`), `snapshot` (the initial root scope); and, from a host that records a running application, `view` (which of its programs this is) and `hashes` (`"templates"` when `program` is taken over the compiled templates instead of the sources: the default is `"sources"`) | First line, and again whenever the program changes (a segment). A player refuses a log whose program or manifest hash is not the one it has |
| `changes` | `changes` (the edits, as `update_changes` takes them, without `base`); `rejects` (a diagnostic code, when the runtime must refuse them) | The edits to the snapshot. A `rejects` line applies nothing: the player requires the diagnostic and keeps the previous render and snapshot |
| `event` | `n` (its sequence number), `on` (the ordinal of the tree it was dispatched against: 0 is the header's, each accepted `changes` line adds one), `handler`, `payload` (when given), `out` (`{ "intent": … }` or `{ "diag": code }`) | An event a PORT reported. Dispatch is pure in (render, handler, payload), so the player dispatches against the render of `on` and requires `out`, compared **without an argument's `name`**: format 1 does not record names (a 0.16 or 0.17 log has none), and a runtime that gives them still replays every log. A handler of an older render is `runtime-unknown-handler` against a newer one |
| `tree` | `sha256`, and `tree` (the whole tree) at every 50th line and at the end | SHA-256 of the tree's compact JSON (members in key order, numbers as MESH writes them), after every `changes` line |
| `result` | `of` (an `event`'s `n`), data | A command's outcome, as data. Informational: the player never reads it, never runs a command, never reads a clock or a random source |

Not recorded: render versions, trees (except at the checkpoints), scroll positions, wall-clock limits.

## The files

Each directory is a program (`program.json`, templates, the shared `components.json` unless it has its own) and:

- `scenario.json`: what to record, written by hand: a `snapshot` and `steps`, each `{ "changes": [...] }` (with `"rejects": true` when the runtime must refuse them) or `{ "event": … }` (found by `component` and `nth` in a tree, or by a literal `handler`);
- `log.jsonl`: the recording, by the native runtime. `MESH_BLESS=1 cargo test -p mesh-runtime --test replay` rewrites every log, and `random.expected.json`; review every rewritten file. A log is regenerated on purpose, with a changelog note, when behavior changes on purpose (R10).

| Directory | Covers |
|---|---|
| `keyed-list` | a keyed list: set, insert, remove, several edits in one step, the whole list replaced, more than 50 lines (checkpoints) |
| `nested-composites` | a composite drawn once per row, its events |
| `else-branch` | `{#if}`/`{:else}` going each way, with edits while a branch is hidden |
| `rejected-update` | changes the runtime refuses, which leave the old render valid, and a later update that applies |
| `numbers` | number text: `0.1`, `1e21`, `5e-324`, the largest double, `-0`, an exact tie |
| `stale-handlers` | a handler of a row that is gone, against the old and the new render, and back |
| `payloads` | a payload on an event that takes none, a foreign and an empty handler |
| `named-arguments` | an event whose argument is a path (`move(item.label)`), recorded without its `name`, which both players give and leave out of the comparison. It has its **own `components.json`**, so the shared one, and every other log, is untouched |

## `random.expected.json`

1,000 random sessions of the `keyed-list` program (12 steps each; the generator is mulberry32, seeded from `seed` and the session's index, and both tests implement it step for step). Each session is played by the three routes, and the digest (the first 16 hexadecimal digits of the SHA-256 of its tree hashes) is recorded. The native test records them; the JavaScript test must give the same ones, so the two runtimes agree on every random session, not only on the routes within each.
