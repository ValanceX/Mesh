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

1. **MESH evaluates MPRX exactly once, in its runtime (I11).** Reactivity lives in the runtime, not in PORT or JavaScript. (One narrow exception, made when the module began returning patches and not the whole tree: the JavaScript package applies the runtime's patches to the previous tree, moving parts by key and reading no value. It is named in the package's review test.) This rules out a signals layer in `port-web`.
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

### A6. Refinement: the host hands over changes, over a structurally shared snapshot

This is a refinement of A3, not a milestone. It lands inside M1 as the second form of `update`'s input, and the full-snapshot form stays (it is the reference and the fallback).

**Why.** The first form of A3 compares the whole new snapshot with the stored one. That is linear in snapshot size, and the JavaScript package must also encode the whole snapshot on every call. The boundary cost dominates for large state.

**The second input form.**

```text
update(render, changes) -> { render', patches } | diagnostics
changes = { base, changes: [{ op: "set" | "insert" | "remove", path, value? }] }
```

- `base` is the **version of the render** the changes apply to (a number the runtime gives every render it makes; see the as-built note below for why it is a version and not a digest of the snapshot). The runtime refuses changes whose `base` is not the held render's version (`runtime-changes-base-mismatch`): changes can never be applied to a snapshot other than the one they were computed against.
- A path addresses a root-scope name and then record fields and list indices. `set` replaces the value at a path (or inserts a record field, or appends at index = length); `remove` removes a record field. Paths are validated against the model's types, and the resulting snapshot is validated exactly as a full snapshot is (I14), so an untyped or ill-formed change is a diagnostic, not a state.
- The runtime **derives** the new snapshot by applying the changes to the held one. **The changes are not a hint beside a snapshot; they define it.** There is no second copy of the state that could disagree with them, so a wrong change set can only produce a *different valid snapshot*, never an inconsistent render.

**Structural sharing in the runtime.** The snapshot's nodes are `Arc`-shared, so applying changes copies only the spine from the root to each changed path and shares every other subtree with the previous snapshot. Consequences:

- A `Render` costs O(changed spine) to create, not O(snapshot). Retaining many renders (time travel, the dispatch rule that a render keeps its own snapshot) is cheap.
- The changed-path set for the patch step is exactly the changes given, normalized (a `set` at `a.b` makes `a.b.c` dirty). With the full-snapshot form, pointer equality (`Arc::ptr_eq`) lets the compare skip any subtree shared with the previous snapshot. Pointer equality only ever short-circuits to "equal"; "different" is always decided by value comparison, so the boundary's equality (binary64 numbers, no `-0`, UTF-16 strings) is unchanged.
- Start with `Arc`-shared records and lists. Adopt a persistent vector (an RRB tree) only if the benchmark below shows plain `Arc<[Value]>` copy-on-write is too slow for big keyed lists; that decision is made in M2 on measurements.

**Making the changes correct by construction and by test.** The host-side producer is the risk, so correctness is enforced in four layers rather than trusted:

1. **Derive changes from states, not from command code.** The host computes `changes` by diffing its previous and next state values, then projecting each state change onto the *scope*: the view's `scope` function runs on the next state, and the diff is taken on scope values. Nothing relies on a command declaring what it touched. Valance does this (M6); the diff function is one shared, specified algorithm with its own tests, not per-app code. How it compares two values is the next subsection.
2. **Chained bases.** Each `update` result is a render with a new version (the as-built note below: a version, not a digest). The host passes it back as the next `base`. A skipped, reordered, duplicated or concurrent update breaks the chain and fails closed with `runtime-changes-base-mismatch`; it can never silently apply.
3. **Verify mode.** A runtime option, on by default in tests, debug builds and the devtools, and off in production, takes the host's full next snapshot alongside the changes and requires `apply(changes, previous) == full` as values (`runtime-changes-disagree` on any difference, with the path of the first). This is the cross-check that the diff producer is right. It costs a full encode, which is exactly why it is not the production default. On the host, the state implied by the changes is also compared with the real next state using the scope Schema's equivalence (below), which reports the differing field.
4. **Property tests as the acceptance gate.** Over generated programs, snapshot pairs and edit sequences, with the Schema equivalence as the independent oracle: (a) `diff(s0, s1)` applied to `s0` yields `s1` exactly; (b) the render and patches from the changes form equal those from the full-snapshot form; (c) the equivalence law of §Non-negotiable rule 5 holds. Edge cases are explicit vectors: list insert, remove and reorder in the middle, an absent-to-present optional field, `-0` to `0`, a field set to its own value (an empty change), a record field renamed (a remove plus a set), and a nested change under a list element that a concurrent change removes.

**How the host diff compares values.** State stays plain data (the Tier 1 rule: plain TypeScript and Effect Schema). The diff walks the scope Schema's structure, so it can produce paths, and decides equality in two ways:

- **Reference shortcut, only on frozen state.** When the committed state is deep-frozen, an unchanged subtree keeps its reference and the diff skips it. Valance freezes committed state in development, tests and verify mode, so an in-place mutation throws where it happens instead of leaving a same-reference, changed-content value that the shortcut would miss. Commands may be written with any technique that returns a new value sharing untouched subtrees, including Immer-style drafts; the diff needs nothing from them beyond that.
- **Schema equivalence otherwise.** Where state is not frozen (production without the guard), the diff compares by value with `Schema.equivalence` generated from the scope Schema. It is slower, since it walks the whole value, but it is always correct and does not depend on how state was produced. It also serves as the oracle in verify mode and in the property tests below.
- **Same equality as the boundary.** Before relying on it, a conformance suite checks that the Schema equivalence agrees with MESH's boundary equality (§9.8.1: binary64 numbers with no `-0`, no NaN or infinities, UTF-16 strings) for every value the boundary admits. Any disagreement is resolved in the host's encoding step, never by loosening the runtime, which remains the final authority on equality.

**When changes cannot be trusted or built:** the host uses the full-snapshot form, which always works. A host never needs the changes form for correctness.

**Benchmark first.** M1 starts with a benchmark and a counter test: a 10,000-item keyed list with one item changed; a deep record with one leaf changed. Reported: bytes encoded across the JavaScript boundary, sites re-evaluated, and wall time, for the full-snapshot form and the changes form. The sharing and changes-form work must show a measured improvement to ship; otherwise it is dropped from M1 and stays as designed.

**Cost model.** Update cost is proportional to the number of changes, the sites that read them, and the spine copy of each changed path. In the full-snapshot form it also includes a compare that skips every shared subtree.

## Part B: composition (the basic-feature gap)

### B1. Children and slots

**Built (decided at review: composite children first).** The design below held, with these differences, found by building it. (1) *No manifest change:* whether a composite takes children is not declared; it takes them exactly when its template contains a `mesh-slot`, since the program, not the manifest, decides what is a composite. (2) *The slot is a step of its own* (kind `0x06`) in the path, as designed. (3) *A node that places a slot is never reused as it was,* because its memo records the names it reads in its own scope and not the caller's content; the content's nodes reuse by their own memos in the caller's scope. A first version without that rule returned stale content in a random update test, which is why the test exists. (4) *Text beside a slot is merged into one run,* to keep a render tree's runs maximal. (5) *A slot may forward:* a slot that is a child of another composite's occurrence passes the content on, through a stack of occurrences the renderer and the dispatch walk both keep. Named slots wait for composite events.

- A composite occurrence may have children. They are **evaluated in the caller's scope** (lexical, as in React children and Angular content projection), never the composite's.
- A composite template places them with a reserved `mesh-slot` element. Named slots (`<mesh-slot name="footer">`, supplied by `slot="footer"` on a child) are included, but only the default slot is in milestone 2.
- Identity: a slotted child's site is its position in the caller's template; the `mesh-slot` site is a site in the composite's template. A key is the pair, so §9.10's "a site, and at a repeated site a key" rule holds without change. A slot inside a `mesh-each` body is keyed by the item key like any other element there.
- The manifest declares whether a composite accepts children, and the compiler checks it (this closes the "children aren't checked against components" known limitation).
- Reactivity needs no special case: slotted content's sites read the caller's paths, and the update table already routes those.

### B2. Composite events

**Built (decided at review: composite events after children).** Differences from the design below, found by building it. (1) *The syntax is not `emit.name(args)`:* MPRX's grammar allows only a plain identifier as a command name, so a dotted callee would need a regenerated parser. The bare form the design weighed against `emit.` is what is built: a handler's name is looked up among the template's commands and then among its component's own events, and a composite may not have an event and a command of one name (rule 6, `assembly-composite-event`, now means this) so the name is never ambiguous. (2) *A forward nothing binds is not in the render tree* (the design left an unhandled forward as a dispatch problem): liveness is decided when rendering, from the occurrence's bindings, outward through every composite, so event resolution (§9.9) goes on past a dead forward and a handler identifier names something that is handled. (3) *Forwards chain through composites and slots:* a record of the occurrences around a handler, with the scopes they were written in, is kept for repeat programs, and derived for the rest. (4) *The declared-events list holds only bindings of commands,* since that is what a host compares with what it handles. Named slots are not built.

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

- Tag names with a hyphen are already valid component names (`tagName` allows `-`), but **event names are identifiers** (`pointSelected`, never `point-selected`), so the renderer's table maps the MESH event to the target's name; building the test found this. The documented rule to add: a hyphenated tag the manifest declares as a primitive is realized by the renderer as an element of that name; MESH assigns it no behavior.
- A manifest for a custom element can type its props (including lists and records, which §9.8.7 only allows natively, never as text) and its events with payloads. The compiler then checks usage as for any primitive. Tooling (hover, completion) works from the manifest with no special case.
- Reactivity is unchanged: a `setProp` on a custom element is a prop change like any other. The PORT side decides which slot receives it (see PORT's design).
- Deliverables (built): a manifest manual section with an example manifest (a `<my-chart>` with a list prop and a `pointSelected` event), and a runtime test that a list and a record prop are carried natively in render-v1 and in `render-patch-v1` `setProp`.

## Status and what building it changed (2026-10-06)

M1, the retained render in the module, and M2's structural patches (Mesh and Port) are built and tested on the `ccr-969a5158-vobqhh` branches; the changes form (A6) is built too, over a structurally shared snapshot. The design above stays the target; this records where the first implementation differs from it, and why.

- **Read sets are derived by the runtime, not emitted by the compiler (A1).** `reads` is a pure function of a template's expressions, so the runtime computes it from the template it is given. That needs no `template-v1` change and no recompile of existing templates, and gives the same answer the compiler would. The compiler change in A1 is dropped unless a second runtime needs the sets without parsing a template.
- **The unit of reuse is a scope name's value, not a root path (A2).** A node's props and a text run's text are recorded with the values of the scope names their expressions read, and reused when those are identical (numbers by their bits, so `0` and `-0` differ). This handles composites and repeated items with no extra mapping, because a composite's or an item's scope is just values under names. It is coarser than a path (a change to any field of a record that is read as a whole re-evaluates), which costs evaluation only, never correctness.
- **Patches come from diffing the two trees (A3 step 4),** which makes the law true by construction, and the random test checks it. Structure is patched by key (see the M2 bullet below).
- **The module keeps renders between calls (decided at review).** `update` keeps the render it returns in a handle table inside the WebAssembly module, and `render.release()` ends a copy (`render()` and `dispatch()` keep nothing; the API review briefly made `render()` keep its result and reverted it, because that broke hosts that never release); a released render or one from a module since replaced updates by deriving itself again, and a render never released is released when it is garbage collected. This departs from "after a call the module holds nothing", and is the one place the module has state; it is documented in the runtime manual, the package README and the changelog, and bounded by a memory test (a chain of ten thousand updates, releasing as it goes). It is also the prerequisite for the table the next bullets describe: a table from a changed value to the nodes that read it needs a retained tree to point into.
- **The O(changes) path, as built (decided at review: "do that next").** The rendered tree is shared (`Rc`), and each node carries a memo: the values of the names its subtree reads from outside it, of the names its own props read, and of each text run's inputs. A repeat remembers its items. So an update keeps a node whose subtree's inputs are identical as the very same node, reuses an identical repeat item without evaluating its key or hashing its path, and the diff skips shared nodes. This is the "table from a changed value to the nodes that read it" the findings above called for, in a different shape: instead of a table pointing into the tree, each node knows what it depends on and an update asks it, top-down, so a subtree that depends on nothing that changed is never entered. Measured (10,000 keyed items, one changed, the least of 15 runs): **7 expressions evaluated, not 60,001; 21 ms, not 91 ms, in Rust; 77 ms, not 481 ms, from JavaScript** (6.2 ms against 47 at 1,000 items). The module returns the patches and not the tree, and the JavaScript package applies them (the narrow exception to I11 noted under the rules). **What it is not:** the cost still follows the size of the snapshot (encode, decode, validate: 12 of the 21 ms in Rust) and, in a changed repeat, one visit per item. Making it follow only what changed needs the changes form below (A6), which this measurement now supports building: the benchmark gate is met in the sense that the snapshot is the remaining cost. It is built (below).
- **Structure is patched by key (M2, built).** The diff matches parts by key and emits removals, then the insertions and moves that put the kept parts in order, then changes inside kept parts, so a list gaining, losing or reordering items is a few operations. A key whose component or event bindings would differ is a remove and an insert, and `replace` remains only for a tree nothing else turns into the next. The equivalence law holds over random snapshots with inserts, deletes, swaps and optional props (1,000 pairs in Rust, against the schema), and end to end: the real runtime's patches, applied by the real Web PORT, give the DOM a fresh draw gives over 4,000 random updates, with every handler still reaching its command. The list diff uses a linear search per out-of-place part, so a worst-case reorder (a reversal) of a very long list is quadratic; a longest-increasing-subsequence pass would make it linearithmic, and the benchmark below does not yet exercise it.
- **Measured (10,000 keyed items, one changed, release build):** a full render evaluates 60,001 expressions in about 169 ms; `update` evaluates 20,005 (the structural `items` and `key` expressions, which M2 addresses) in about 149 ms, with 2 patches. Program validation is 0.3 ms and cloning a render is 27 ms of that. **The time is in rebuilding and re-keying the whole tree** (each node's key is a SHA-256), not in evaluation, so reusing results alone gives a third of the evaluations but only a small share of the time.
- **The changes form (A6), as built (decided at review: "build the changes form").** `update_changes(previous, changes, verify?)` in Rust and `updateChanges(render, changes, { verify })` in the package apply `set`, `insert` and `remove` edits at paths to the previous render's validated values (`Rc`-shared, so what an edit doesn't touch is the same value), validate only the values the edits give, and build the tree with the same memos as `update`. The edits define the new snapshot. Differences from the design above, each decided in the build:
  - **`base` is a version, not a digest.** Every render the runtime makes takes the next number of a counter; changes name the render they were computed against. It costs nothing (a digest of the snapshot would read it all, which is what the form avoids) and refuses the same mistake. It is not content-addressed: two renders of one snapshot have different versions, and the host passes the version the render reports (`render.version`).
  - **The operations are `set`, `insert` and `remove`,** not `set` and `remove` alone: a list insert or removal in the middle shifts later indices, and as a `set` per later element it would cost the size of the list.
  - **Verify mode** (`verify` with the whole snapshot the host believes it has): validates it and compares it with what the changes make, refusing with `runtime-changes-disagree` at the first differing path. It costs a validation of the whole snapshot, so it is for tests and development; the package's tests run their chains with it.
  - **`diff(previous, next)` is in the JavaScript package,** as a host-side utility for a host that has two snapshots and no knowledge of what changed. It reads host values, which the review test (I11) allowed only the encoder to do: the test now names `changes.ts` beside `patches.ts`, and checks that nothing the runtime does imports it and that it makes no text and no number.
  - **A render made from changes has its snapshot only in the module.** There is no encoded copy in the host (the point of the form), so such a render dispatches and updates while the module holds it, and after `release()` or a replaced module it throws a `TypeError` and the host renders afresh.
  - **Measured (10,000 keyed items, one changed):** 7 expressions evaluated; 13 ms in Rust, against 21 to 42 for a whole-snapshot update and 91 to 114 to render; **22 to 26 ms from JavaScript, against 60 to 64 for the whole-snapshot update and 460 to 470 to render** (1.7 ms against 5.5 at 1,000 items). What remains is one visit per item of the changed repeat, which a table from a changed value to the nodes that read it (the original A2 table) would remove; that is not built, and the numbers don't yet call for it.

## Milestones

Each milestone is releasable and carries its own conformance vectors.

| # | Milestone | Repos | Acceptance |
|---|---|---|---|
| **M1** | Static `reads`, `update` (full-snapshot form, then the A6 changes form over an `Arc`-shared snapshot, gated on the benchmark) and `render-patch-v1` for **props and text only** (no structural change), plus PORT's patch application | Mesh, Port | Equivalence law holds in a property test over generated programs and snapshots; a one-field change touches one DOM node in a Chromium test; untouched nodes are not re-evaluated (counter test) |
| **M2** (built; see Status) | Structural patches: `insert`, `remove`, `move` for `mesh-if` and keyed `mesh-each`; per-item reuse | Mesh, Port | Identity vectors (§9.10) still pass; reorder of 1,000 keyed items moves nodes and reuses every node object |
| **M3** | Expression gaps: `len`, `has`, `?.`, subscripts, `mesh-if` else and nesting; `mesh-each` and `mesh-if` become M3 | Mesh | Spec §9.7 text, diagnostics with stable codes, LSP completion and hover cover them |
| **M4** | Children and a default `mesh-slot`; manifest `children` declaration and checking | Mesh, Port | Composite with children renders; identity vectors extended; update routes caller paths through a slot |
| **M5** | Composite events (`emit`) and named slots | Mesh | Dispatch forwarding vectors; at-most-one-binding rule holds |
| **M6** | Valance and Nexus integration: `host.render` becomes `update` when the program is unchanged, with the shared state-diff producing A6's changes and versioned bases; the Valance Web host applies patches | Valance, Nexus | docs-site example updates fine-grained end to end; documented in Tier 1 |

| **M3a** | Error handling: diagnostic codes for every new construct, previous-render-intact guarantee for failed `update`, documented failure-as-state host pattern with vector | Mesh | A failed `update` returns diagnostics only and the old `Render` still dispatches; codes are in the diagnostics manual |
| **M1b** | `explain` output and the versioned session-log format with a replay vector | Mesh | Replay of a recorded log reproduces every tree byte for byte |
| **M1c** (built in Rust; the vector in JavaScript is not) | Web Component manifest guide, example manifest and native list/record conformance vectors | Mesh | Vectors pass in Rust and JavaScript |
| **M7** | Devtools panel: command log, time travel, patch and dependency inspector | Valance, Nexus | Jumping to any past step restores that UI; a node can be traced to its state path and MPRX span |

Ordering note: M1 comes before the language additions on purpose. The patch format and equivalence law constrain every later construct, so they must exist before structure grows.

## Risks

- **Snapshot compare and encode cost** on very large state. Mitigated by the changes form over a structurally shared snapshot (A6), with its correctness layers.
- **A wrong change set.** Cannot corrupt a render, since the changes define the new snapshot and are type-validated, but could show the wrong state. Mitigated by deriving changes from state diffs, versioned bases and verify mode (A6).
- **`reads` conservatism.** A coarse `reads` only costs extra re-evaluation, never correctness, because the equivalence law is tested against full render.
- **Pre-1.0 churn.** Port declares `peerDependencies` on a MESH range; each MESH minor needs a Port range bump. M1 touches both, so release them together and bump the ranges in the same change.
- **Cross-repo coupling.** PORT must not depend on the MESH runtime (contract obligation 7). It consumes the patch *schema* only, as it does `render-v1` today.

## Out of scope

A router (a stated Valance non-goal; a separate decision), a dev server, and Valance's Tier 2 surface. Accessibility and styling are PORT's own roadmap.
