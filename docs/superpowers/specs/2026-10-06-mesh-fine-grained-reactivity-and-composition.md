# MESH: fine-grained reactivity and composition (design)

Status: **proposal for review** (2026-10-06). Nothing here is implemented. Companion: PORT's [patch application design](https://github.com/ValanceX/Port/blob/main/docs/superpowers/specs/2026-10-06-port-web-patch-application.md).

## Goal

Make the Valance stack match the *basic* feature set of React and Angular for a single-page app, with **fine-grained reactivity**: a state change re-evaluates and touches only the parts of the UI that read what changed.

Starting point (MESH 0.9.0, from the feature-coverage doc and the source):

- `render` evaluates the whole program against a whole snapshot and returns a whole render-v1 tree. There is no incremental operation.
- PORT keeps node identity by key and reconciles the full new tree against the drawn one. That is virtual-DOM granularity.
- `mesh-if` and `mesh-each` are provisional (M2), cannot nest `mesh-if` directly, and have no else branch.
- Composites have no slots, no children and no events (`assembly-composite-event`).
- MPRX has no length or presence operator, no subscripts, no optional chaining.

## Non-negotiable rules (kept from the architecture)

1. **MESH evaluates MPRX exactly once, in its runtime (I11).** Reactivity lives in the runtime, not in PORT or JavaScript. This rules out a signals layer in `port-web`.
2. **Renderer independence.** Patches carry no DOM concepts.
3. **Identity is §9.10.** A patch names nodes by the keys §9.10 already defines. No new identity scheme.
4. **Additive evolution.** `template-v1` and `render-v1` gain optional properties only; a new patch format gets its own schema. Pre-1.0 minor breaks stay allowed but are not needed.
5. **Equivalence law.** For any program, snapshots *s₀*, *s₁*: applying `update(render(s₀), s₁).patches` to `render(s₀).tree` yields exactly `render(s₁).tree`. This is testable by property tests and is the acceptance gate for every reactivity milestone.

## Part A: dependency-tracked patches

### A1. Static read sets in the template

MPRX expressions are pure and have no side effects, so what an expression reads is knowable at compile time. The compiler adds an optional `reads` array to every expression, in `template-v1`: the root-scope paths the expression may read, as `["user", "name"]` for `user.name`.

- Static, not tracked at run time: no proxies, no subscription bookkeeping, and the same answer on every target.
- Conservative: `a && b` and `c ? x : y` read all of their operands' paths. A read of `user` as a whole subsumes `user.name`.
- Composite props are expressions, so a composite's scope names map to the paths its props read; the compiler composes these across a composite boundary so a root-scope path reaches every site below it.
- Old templates without `reads` still run: the runtime treats a missing `reads` as "reads everything" and falls back to a full tree (A4).

### A2. Binding sites

A **binding site** is any place an expression's value reaches the output: a prop, an event argument, a text run, a `mesh-if` condition, a `mesh-each` items expression or key. §9.10's sites already enumerate these. The runtime keeps, per `Render`, a table from root-scope path to the binding sites that read it.

### A3. `update`

A new third runtime operation next to render and dispatch:

```text
update(render, snapshot) -> { render', patches } | diagnostics
```

1. Validate `snapshot` exactly as `render` does (I14).
2. Compare it with `render`'s stored snapshot by structural equality of boundary values (§9.8.1), which gives the set of **changed paths**. A change at `user.name` does not dirty `user.age`.
3. Re-evaluate only binding sites whose `reads` intersect a changed path. Unchanged sites keep their values and emit nothing.
4. Emit patches for sites whose value changed. A structural site (`mesh-if`, `mesh-each`) emits insert, remove and move patches by key.
5. `render'` is a full `Render` (tree, program, model and the new snapshot), so `dispatch` keeps working on it, and the host keeps the one-render-per-drawn-tree obligation from the runtime manual.

`render` stays the entry point for a new or different program; `update` is only valid for the same program (the composer's *program continuity* fact, unchanged).

### A4. `render-patch-v1`

A new JSON Schema, `schemas/render-patch-v1.schema.json`. A patch list is ordered and applied in sequence:

| op | fields | meaning |
|---|---|---|
| `setProp` | `key`, `prop`, `value`, `propText?` | the prop now has this value (and text), as `render-v1` would write it |
| `removeProp` | `key`, `prop` | the prop is now absent |
| `setText` | `key`, `text` | a text child's text |
| `insert` | `parent`, `before?`, `node` | a whole render-v1 node (or text), subtree included, new under `parent`, before sibling `before` or last |
| `remove` | `key` | the node and its subtree are gone; its key may later return as a new object |
| `move` | `key`, `before?` | same node, new sibling position (keyed `mesh-each` reorder) |
| `replace` | `tree` | escape hatch: this whole tree, equivalent to `draw`. The runtime may always emit it (for example, for an old template with no `reads`) |

Rules:

- Event handler identifiers are identity-derived (§9.10), so they are stable across updates. Events change only on `insert`; there is no `setEvent` op. If a later construct needs one, it is additive.
- Patches never mention composite boundaries (they are not in the tree).
- A patch list is **minimal in effect, not unique in form**: the equivalence law constrains the result, not the encoding. Conformance is by applying, never by comparing patch lists.

### A5. Keyed lists

`mesh-each` items are tracked by their declared key. For each key present in both renders: if the item value is structurally unchanged, nothing is evaluated for it; if it changed, only the sites inside the body that read the item's changed fields re-evaluate. New keys `insert`, missing keys `remove`, reordered keys `move`. Duplicate or invalid keys still fail closed (§9.10).

### A6. Cost model

Update cost is proportional to the number of changed paths plus the sites that read them, plus a structural compare of the snapshot. The compare is a full walk of the snapshot, which is linear in snapshot size and cheap compared to evaluating a template. A later refinement lets the host hand over changed paths (a hint, verified under test, never trusted for correctness).

## Part B: composition (the basic-feature gap)

### B1. Children and slots

- A composite occurrence may have children. They are **evaluated in the caller's scope** (lexical, as in React children and Angular content projection), never the composite's.
- A composite template places them with a reserved `mesh-slot` element. Named slots (`<mesh-slot name="footer">`, supplied by `slot="footer"` on a child) are included, but only the default slot is in milestone 2.
- Identity: a slotted child's site is its position in the caller's template; the `mesh-slot` site is a site in the composite's template. A key is the pair, so §9.10's "a site, and at a repeated site a key" rule holds without change. A slot inside a `mesh-each` body is keyed by the item key like any other element there.
- The manifest declares whether a composite accepts children, and the compiler checks it (this closes the "children aren't checked against components" known limitation).
- Reactivity needs no special case: slotted content's sites read the caller's paths, and the update table already routes those.

### B2. Composite events

Today a composite has no events. Proposal: a composite's manifest entry may declare events with a payload type, and its template forwards a primitive's event to one of them:

```xml
<!-- the composite's template -->
<button on.click={emit.select(item.id)} />

<!-- the caller -->
<user-card item={u} on.select={selectUser($event)} />
```

`emit.name(args...)` is not code. It is a declared forward: when the primitive's event resolves, the runtime continues resolution at the composite's occurrence for the event `name`, with the evaluated args as its payload. This keeps §9.9's "at most one binding per interaction" and the rule that renderers see only the leaf handler: dispatch performs the forwarding, never PORT.

**Open question for review:** the `emit.` spelling versus a bare `on.click={select(item.id)}` where `select` is a declared composite event rather than a command. The first is explicit and cannot clash with a command name; the second reads closer to JSX. I recommend `emit.` for the first cut because it is greppable and costs nothing to relax later.

### B3. Stabilizing conditionals and repetition

`mesh-if` and `mesh-each` go from M2 to M3 by closing the gaps the coverage doc lists:

- **Else branch and nesting.** `mesh-if` takes `then` and an optional `else` child element; direct `mesh-if` nesting is allowed once identity for nested conditional sites is specified (it follows from §9.10's position rule; the spec text is the work).
- **Presence and length.** Add three pure expression forms: `len(x)` for a list or string, `has(x)` for presence of an optional value, and `x?.y` optional member access. These remove the "compute an empty flag in scope" workaround and the missing presence test noted under Known limitations.
- **Subscripts.** `list[i]` with a bounds failure as a runtime diagnostic, not `null`.
- **Rename.** Keep the reserved names `mesh-if` and `mesh-each` as the stable spelling. A prettier surface can be sugar later; renaming now would cost every example.

## Part C: devtools (replay and time travel)

State changes only through commands, and render is pure: no I/O, clock, randomness or global state (runtime invariants). So a session is fully described by an ordered log, and replay needs no instrumentation of application code.

- **The log is data.** A session is `(program, model, initial snapshot)` plus the ordered `(handler, payload)` dispatches the host relayed, plus the command results Nexus committed. MESH's part: the runtime already returns intents deterministically, so the same log gives the same intents and, through `render`/`update`, the same trees.
- **Time travel is `render` of a past snapshot.** Jumping to step *n* renders the snapshot recorded at *n* with `render` (a `replace`/draw for the renderer), never `update`, so it works across any history gap. Stepping forward one command uses `update` and shows the real patch list, which is itself a devtools feature: the panel can show *why* a node changed (the changed path, the binding site that read it, the patch).
- **Inspection comes from existing data.** Template `reads` (A1) give a dependency graph: which state path feeds which site. Keys and handler identifiers (§9.10) give stable node identity across frames. Spans in templates map any rendered node back to MPRX source.
- **MESH deliverables:** (1) `render-v1` and `render-patch-v1` JSON are the only formats a recorder needs, so a recorder is a host concern, not a runtime feature; (2) an optional `explain` output from `update` (changed paths, sites re-evaluated, sites skipped), off by default and not part of the equivalence law; (3) a documented, versioned **session log** format with a conformance replay vector: replaying a recorded log reproduces every recorded tree byte for byte.
- **Not in MESH:** the UI of the devtools, and recording of Nexus command results. Those belong to Nexus and Valance (milestone M7).

## Part D: error handling

Principle: **a failure is state, and an invalid program fails closed with diagnostics.** MESH already follows the second half; this part makes both explicit and checks they hold for everything this design adds.

- **Fail closed.** `render` and `update` return diagnostics or a result, never both and never an exception (runtime manual). Every new construct gets stable diagnostic codes (never renamed or reused): `reads` inconsistent with an expression, malformed patch lists, `emit` of an undeclared event, children given to a composite that declares none, slot misuse, and out-of-bounds subscripts. A failed `update` leaves the previous `Render` valid and unchanged, so the drawn UI never diverges from a render the host holds.
- **No silent fallback hiding a bug.** The `replace` escape hatch (A4) is allowed only for missing `reads` or an explicit runtime decision, and `explain` reports why it was used. A patch that would not satisfy the equivalence law is a runtime bug, not a condition to recover from.
- **Failure as state, in templates.** MPRX stays free of exceptions and handlers. The way to express a failure in the UI is the one already used elsewhere: the host models it in scope (for example a `status` record with `loading`, `error` and `data`), and the template shows it with `mesh-if`. This design adds what that pattern needs: `has(x)` and `?.` (M3) so absent data is testable, and `else` so the error branch is one conditional. Nexus owns producing the failure state from a failed command; MESH only renders it.
- **Template-level boundary (proposal).** Because rendering a failed expression (a subscript out of bounds, a prop of the wrong type) currently fails the whole render, `mesh-if` gains no catch semantics, but the host can keep the last good render and surface the diagnostics as its own error state. Documenting this host pattern, with a conformance vector, is in M3. A per-subtree error boundary is deliberately not proposed: it would let invalid output render, against the fail-closed rule.
- **Dispatch.** An unknown or stale handler identifier is already a diagnostic (`§9.10.7`); patches add nothing here, since identifiers are identity-derived and stable.

## Part E: Web Components as primitives (MESH side)

A custom element is just a primitive: the manifest declares its tag, props, events and payload types. MESH needs no change for this, and the design keeps it that way.

- Tag names with a hyphen are already valid component names (`tagName` allows `-`). The documented rule to add: a hyphenated tag the manifest declares as a primitive is realized by the renderer as an element of that name; MESH assigns it no behavior.
- A manifest for a custom element can type its props (including lists and records, which §9.8.7 only allows natively, never as text) and its events with payloads. The compiler then checks usage as for any primitive. Tooling (hover, completion) works from the manifest with no special case.
- Reactivity is unchanged: a `setProp` on a custom element is a prop change like any other. The PORT side decides which slot receives it (see PORT's design).
- Deliverables: a manifest guide section and an example manifest (a typical `<my-chart>` with a record prop and a `point-selected` event), plus a conformance vector that a list/record prop is carried natively in render-v1 and in `render-patch-v1` `setProp`.

## Milestones

Each milestone is releasable and carries its own conformance vectors.

| # | Milestone | Repos | Acceptance |
|---|---|---|---|
| **M1** | Static `reads`, `update` and `render-patch-v1` for **props and text only** (no structural change), plus PORT's patch application | Mesh, Port | Equivalence law holds in a property test over generated programs and snapshots; a one-field change touches one DOM node in a Chromium test; untouched nodes are not re-evaluated (counter test) |
| **M2** | Structural patches: `insert`, `remove`, `move` for `mesh-if` and keyed `mesh-each`; per-item reuse | Mesh, Port | Identity vectors (§9.10) still pass; reorder of 1,000 keyed items moves nodes and reuses every node object |
| **M3** | Expression gaps: `len`, `has`, `?.`, subscripts, `mesh-if` else and nesting; `mesh-each` and `mesh-if` become M3 | Mesh | Spec §9.7 text, diagnostics with stable codes, LSP completion and hover cover them |
| **M4** | Children and a default `mesh-slot`; manifest `children` declaration and checking | Mesh, Port | Composite with children renders; identity vectors extended; update routes caller paths through a slot |
| **M5** | Composite events (`emit`) and named slots | Mesh | Dispatch forwarding vectors; at-most-one-binding rule holds |
| **M6** | Valance and Nexus integration: `host.render` becomes `update` when the program is unchanged; the Valance Web host applies patches | Valance, Nexus | docs-site example updates fine-grained end to end; documented in Tier 1 |

| **M3a** | Error handling: diagnostic codes for every new construct, previous-render-intact guarantee for failed `update`, documented failure-as-state host pattern with vector | Mesh | A failed `update` returns diagnostics only and the old `Render` still dispatches; codes are in the diagnostics manual |
| **M1b** | `explain` output and the versioned session-log format with a replay vector | Mesh | Replay of a recorded log reproduces every tree byte for byte |
| **M1c** | Web Component manifest guide, example manifest and native list/record conformance vectors | Mesh | Vectors pass in Rust and JavaScript |
| **M7** | Devtools panel: command log, time travel, patch and dependency inspector | Valance, Nexus | Jumping to any past step restores that UI; a node can be traced to its state path and MPRX span |

Ordering note: M1 comes before the language additions on purpose. The patch format and equivalence law constrain every later construct, so they must exist before structure grows.

## Risks

- **Snapshot compare cost** on very large state. Mitigated by the host-supplied changed-path hint (A6), later.
- **`reads` conservatism.** A coarse `reads` only costs extra re-evaluation, never correctness, because the equivalence law is tested against full render.
- **Pre-1.0 churn.** Port declares `peerDependencies` on a MESH range; each MESH minor needs a Port range bump. M1 touches both, so release them together and bump the ranges in the same change.
- **Cross-repo coupling.** PORT must not depend on the MESH runtime (contract obligation 7). It consumes the patch *schema* only, as it does `render-v1` today.

## Out of scope

A router (a stated Valance non-goal; a separate decision), a dev server, and Valance's Tier 2 surface. Accessibility and styling are PORT's own roadmap.
