# Ecosystem API design review

Status: **closed** for this round; the decisions are in the sections below, and the items still open are listed at the end. This is a record of the review, not a specification; the behavior is in the manuals and the stability pages.

Standard: [API Design Principles v1.0](../../standards/api-design-principles.md). Baseline: [the freeze](./2026-10-09-api-review-baseline.md). Reviewed 2026-10-09 on the `ccr-969a5158-vobqhh` branches. Mesh and Port were reviewed and changed by the author of this note; Nexus and Valance were first read by two reviewers who changed nothing, and the findings below were checked against the cited code only where noted. The findings marked done in those two repos were made after installing dependencies and running typecheck and tests.

How to read it: each finding has a principle, the evidence, a disposition, and whether it would break a released surface. **Done** means changed and tested in this review. **Owner** means a judgment call that is not made here. Released means Mesh 0.9.0, Port as published, Nexus 0.10.3, Valance 0.5.0.

## Mesh

| # | Principle | Finding | Disposition |
|---|---|---|---|
| M1 | 9.1 | Thrown errors had no stable code: `MeshInternalError`, `MeshVersionError` had none, and bare `TypeError`s were thrown for misuse. | **Done.** `code` on both; new `MeshUsageError extends TypeError` (`invalid-argument`, `not-a-render`, `render-gone`), in both JS packages. Additive: `instanceof TypeError` still holds. |
| M2 | 7 | A `Render` holds a resource (its copy in the module) but offered only `release()`. | **Done.** `[Symbol.dispose]()` where the platform has it, tested. The repo's Node 22 can't parse `using`, so the test calls the method. |
| M3 | 8 | Serialization of calls and the absence of cancellation were stated in the manual but not in the package's contract. | **Done** (README, manual): calls run one at a time in order, no cancellation, nothing continues after a promise settles except queued release. |
| M4 | 12, 15.4 | No page said which surfaces are stable, unreleased, provisional or internal. | **Done.** [API stability](../../manual/stability.md), linked from the docs index and the changelog. |
| M5 | 4.5 | `Changes.base` and `Render.version` were plain `number`; a version could be confused with any number. | **Done.** `RenderVersion`, a branded number type, exported; `render.version` returns it and `Changes.base` takes it. Type-level only: a host that builds changes from JSON casts once, at the boundary it owns. |
| M6 | 2, 2.2 | `update` and `updateChanges` are two verbs for one operation; `render({ keep })` decided whether `updateChanges` was possible later. | **Done, then corrected by G1.** `keep` is gone. Making every render kept (the first attempt) broke hosts that never release; the final shape is that `render()` keeps nothing and `updateChanges` accepts any render, deriving it when the module doesn't hold it. The two verbs stay: they take different inputs, and an overload by input shape is what 15.2 warns about. |
| M7 | 4.4, 9.2 | `render.version` is `undefined` once released; use-after-release throws (`render-gone`) where most failures return diagnostics. | **Owner.** The throw is a programmer error and now has a code; the alternative is a `runtime-render-released` diagnostic. Kept as thrown because a diagnostic document is for problems with the program or snapshot. |
| M8 | 3.3 | `Render` names an entity after a process. | **Not changed.** Released in 0.9.0; a rename is a migration, not a fix. Noted for a major version. |
| M9 | 14, 12 | `diff` is exported from the runtime package though the runtime never uses it. | **Kept.** It is the host-side producer of `changes`; separating it into a package would add a dependency for the one consumer that needs it. Its tier and the review test that confines it are documented. |
| M10 | 9.1 | Runtime diagnostics had `code`, `message`, `location` but no corrective action. | **Done for the ten codes where one action reliably applies** (a per-code `hint`, optional in the schema); not added where the right action depends on the case. |
| M11 | 3.1, 17 | Rust and JavaScript names follow each language's convention (`update_changes` / `updateChanges`); `host_snapshot` has no JavaScript counterpart. | **Fine.** No change. |
| M12 | 15.4 | The module's `mesh_*` exports are internal but listed beside the public ones in package source. | **Done** by the stability page (internal tier). The package already checks for them and documents them as internal. |
| M13 | 13.2 | README and manual examples are not compiled or run. | **Owner.** Worth a test that extracts and runs the package README's snippets; not built. |
| M14 | 11 | Cost is documented for update and changes (measured); `render` of a large program is not. | **Fine for now.** The manual's table states render and update costs. |

## Port

| # | Principle | Finding | Disposition |
|---|---|---|---|
| P1 | 7 | `unmount()` had no `Symbol.dispose`, and its idempotency and post-unmount behavior were unstated. | **Done.** Documented (safe twice, usable again via `draw`, `update`/`patch` refuse with `not-drawn`); `[Symbol.dispose]` where available, tested. 312 tests pass. |
| P2 | 9.1 | `WebRealizationError` already has a stable `code`, a `key` and a `cause`. | **Good.** No change. |
| P3 | 15.4 | `inspect` and `patch`'s returned keys are "not part of the contract" in the docs. | **Good**, and a model for the other packages' tiers. |
| P4 | 17 | `createWebPort` takes `{ container, primitives, report }` (one object, three required). | **Good.** |

## Nexus (read by a reviewer; nothing changed)

| # | Principle | Finding | Breaking? | Disposition |
|---|---|---|---|---|
| N1 | 13.2, 4.5 | `docs/primitives/state.md` example uses `Schema.OptionFromNullOr`, which `State.create`'s `Schema<A>` doesn't accept; `examples/basic-app` uses `OptionFromSelf` with a note saying so. | doc-only | **Owner**: fix the doc example. Not edited because nothing could be compiled here. |
| N2 | 4.5, 9.4 | `Schema.Schema<A>` parameters (state, command, event, application) reject any transforming schema, and `State` decodes already-typed values. | widening is additive; State behavior change is behavioral | **Owner.** |
| N3 | 6.2, 8, 7 | `Runtime.run`/`runFork` work isn't counted in admission, so `shutdown` doesn't wait for it and releases resources under it; docs don't say so. | doc: none; drain: behavioral | **Owner**: at least document; an opt-in drain is additive. |
| N4 | 9.1, 9.2 | Misuse and use-after-shutdown failures are plain `Error("NEXUS: …")`; callers string-match. | additive (a tagged class) | **Owner.** The `_tag` convention on typed errors satisfies 9.1; a tagged refusal would too. |
| N5 | 5.2, 15.4 | `@valancex/mesh-runtime` is a non-optional peer and imported at load by the root export `Mesh`, though docs say only `Mesh.host` users need it. | doc-only, or additive subpath | **Owner.** |
| N6 | 12 | `effect` is a `dependency` though its types are the public surface. | packaging | **Owner**: probably a peer dependency. |
| N7 | 15.4 | No stability tiers; `Semantic`, `Mesh` and `EventBusShape` are exported with the stable primitives. | doc-only | **Owner**; the Mesh stability page is a template. |
| N8 | 3.2, 2.2 | Vocabulary: `name`/`id`/`_tag`; `scope`; duplicated handle-member and namespace-function forms; "nine primitives" vs eleven namespaces. | renames breaking | **Owner**, major version only. |
| N9 | 9.4 | `Event.subscribe` matches on `_tag` alone and casts the payload; tag collisions cross-deliver. | additive | **Owner**: document, or opt-in validation. |
| N10 | 9.4 | `State.update` bypasses the schema; README says state is validated. | additive/doc | **Owner.** |
| N11 | 4.4, 9.1 | `issues: [String(error)]` loses causes; `Runtime.run` rejects with Effect's FiberFailure wrapper. | additive | **Owner.** |
| N12–N15 | various | `Mesh.bind` examples cast arguments; under-documented lifecycle details (unbounded `PubSub`); stale README Status; no `Symbol.dispose` guidance. | doc/additive | **Owner.** |

## Valance (read by a reviewer; nothing changed)

| # | Principle | Finding | Breaking? | Disposition |
|---|---|---|---|---|
| V1 | 11, 6.2 | `pending` fibers accumulate per reported event unless `settled` is called; `dispatched` grows by design. | the `pending` fix is internal; bounding `dispatched` breaks documented behavior | **Owner**: fix `pending`, decide `dispatched`. Reasoned from code, not measured. |
| V2 | 4.4, 5.2 | Server render returns HTML and a separately read state that can differ after start work commits; start work runs with no opt-out. | returning the rendered-from state is behavioral | **Owner.** |
| V3 | 9.2, 9.3 | A failing start command's exit is discarded and not logged. | additive | **Owner.** |
| V4 | 9.1, 17 | Defects are plain `Error`s with message prefixes (the contract tells callers to rely on the text); `Web.run` rejects with plain `{_tag}` objects. | defect class additive; changing what `run` rejects with is risky | **Owner.** |
| V5 | 4.5 | `ApplicationHandle` is structural but the runtime requires a registered handle. | type-level | **Owner**: brand it. |
| V6 | 4.5, 6.1 | `invoke(key: string, args)` is untyped and callers hand-wrap MESH argument objects. | additive | **Owner.** |
| V7 | 12, 15.4, 4.5 | `Mounted.dispatched` has an unnameable type; `./internal` is published with its tier only in prose. | additive | **Owner.** |
| V8–V10 | 6.1, 6.3, 4.5 | Users hand-wire MESH `init`, state embedding and `history.window`; view names and scope schemas are restated for `manifest` and `define`; `event.of` forces element casts. | additive | **Owner.** |
| V11–V14, V17 | various | Vocabulary collisions (`Host`, `scope`, `start`); stale-run cancellation; unchecked `define` and `present` typo; no `Symbol.asyncDispose`; module-level registries. | aliases additive; renames breaking | **Owner.** |
| V15 | 13 | Docs omit `./web/build`, hand-write `components.json`, and leave types out of the reference; snippets aren't compiled. | doc-only | **Owner.** |
| V16 | 6.1, 14.1 | Composition: each state does a full MESH render and gives PORT a whole tree; `update`/`updateChanges`/`diff`/`version`/`release` and PORT `patch` would remove that. `Target` is user-implementable, so any new member must be optional. | additive | **Owner**; this is milestone M6 of the reactivity design. |

## What was done in Nexus and Valance, and what was not

Dependencies were installed and each repo's typecheck and tests run before and after (Nexus 494 then 495 tests, Valance 114, all passing).

- **N3 (done, as documentation and a test).** The claim was checked: a probe showed `terminate` completing while a `runFork` effect was still running, which then finished after the scope closed. The runtime manual now says termination does not wait for running effects and that a resource they use can be released under them, and a test pins it. Changing `shutdown` to wait would be a behavior change that can hang a shutdown on a long-lived fork, so it is left to the owner.
- **N1 (done).** The State example now uses `Schema.OptionFromSelf`, as `examples/basic-app` does. `docs/ARCHITECTURE.md` has an older sketch of the same example with a different `State.create` shape; it was left as a design record.
- **V1 (done).** A finished dispatch leaves a mount's `pending` list, so a page that never calls `settled` no longer holds every fiber. Reasoned from the code and not measured; `dispatched`, the ledger the contract says is never trimmed, is unchanged.
- **N4 (not done).** Nexus's `refusal` is documented in its source as "deliberately not a public error type", so adding one reverses a stated decision. It stays with the owner.
- **V3 (not done).** The startup-work guide and the contract say Valance has no startup error channel and writes nothing to the log; the state is the channel. Logging the failure would reverse that. It stays with the owner.
- **Everything else** in the Nexus and Valance tables is unchanged and stays with the owner: those fixes change types, packaging, public names or documented behavior, or add API.

## What the review changed

Mesh: error codes and `MeshUsageError` in both JavaScript packages, `Symbol.dispose` on `Render`, the concurrency and error contract in the READMEs, manual and guide, and the stability page. Port: `unmount` contract and `Symbol.dispose`. Tests: Mesh runtime 95 passing (4 new), compiler 46, Port 312 (1 new). No released behavior changed.

## Decisions taken ("go for recommended")

1. M6: `keep` removed; `updateChanges` accepts any render; two verbs stay. Done (corrected by G1 in the second pass).
2. M5: `RenderVersion` brand. Done.
3. Nexus and Valance: the recommended order was N3, V1, V3, N4, N1 first; N3, V1 and N1 are done, N4 and V3 conflict with documented decisions and stay with the owner (below).
4. M10: hints on the common codes. Done.

## Second pass: the changes made since, checked against the principles (2026-10-09)

Scope: everything changed after the first review: `Render` always kept, `RenderVersion`, `hint`, `MeshUsageError` and codes, `Symbol.dispose`, `Runtime.Refusal`, start-work logging, the `pending` fix. The standard is unchanged (only its status line now reads "proposed standard"). The shutdown design is deferred.

### Gaps found

| # | Principle | Gap | Evidence | Severity | Recommendation |
|---|---|---|---|---|---|
| G1 | 15.2, 15.1, 11, 7 | **`render()` is a Stable API (0.9.0) and now retains its result in the module until released or garbage collected.** A host that renders and never releases, which was correct and documented before, now holds module memory. The JS garbage collector can't see that memory, so a tight loop exhausts it before finalizers run. | Measured: a 500-item snapshot costs about 1.2 MB per retained render; a loop of `render()` with no release fails with `MeshInternalError` after 1,796 renders at 2.15 GB. With 300 renders and the GC given turns, all 300 were released, but the module's linear memory (362 MB) does not shrink. | **Blocks a release (resolved in the third pass).** A lifecycle change on a stable call is the case 15.2 says is never automatically non-breaking. M6 option B (my recommendation, taken) was wrong for `render`. | Make `render()` keep nothing again, as released. Keep retention for what is new: `update` and `updateChanges` retain the render they return (unreleased APIs, so they may define their own lifecycle). Let `updateChanges` accept any render by deriving it, so no `keep` flag is needed: give every render a version when it is made and let the module adopt that version when it derives. This restores the released contract, removes the option M6 wanted removed, and costs one derive on the first `updateChanges` from a plain render. Needs your decision before it is built. |
| G2 | 15.2, 15.4 | The `hint` property was added to `runtime-diagnostics-v1`, whose schema has `additionalProperties: false`. A consumer validating strictly against the old schema rejects new documents. The stability page calls the schema Stable. | `schemas/runtime-diagnostics-v1.schema.json`; the changelog notes it. | Should fix before release | Either state in the stability page and schema description that v1 documents may gain optional properties and consumers must ignore unknown ones, or ship `hint` under a documented schema revision. The first is cheaper and matches how codes already grow. |
| G3 | 3.2, 9.1, 2.2 | The ecosystem now has four spellings of an error's stable identity: Mesh and Port use `code` on `Error` subclasses, Nexus's `Refusal` uses `reason`, Valance and Nexus typed errors use `_tag`, Valance defects use message prefixes. `_tag` for the Effect error channel is a convention worth keeping; `reason` on an `Error` subclass is the odd one. | `Runtime.Refusal.reason`; `MeshUsageError.code`; `WebRealizationError.code`; Valance V4. | Should fix while `Refusal` is unreleased | Rename `reason` to `code` (keeping `_tag` for channel errors), and write the rule once: thrown or defect errors carry `code`, channel errors carry `_tag`. Valance's defects follow when it can depend on a Nexus that has `Refusal` (V4). |
| G4 | 9.3, 10 | The start-work failure is logged with its whole `cause`. A command's failure can carry request payloads or user data. The principles forbid sensitive payloads in logs. The same applies to the existing popstate logging. | `packages/valance/src/index.ts` (`Effect.logError("start-time work failed", cause)`) | Should fix | Log the failure's `_tag` or message and a pretty-printed cause only at debug level, or say plainly in the docs that the cause is logged and applications must keep secrets out of errors. The second is the minimum. |
| G5 | 15.4, 12 | Only Mesh has a stability page. `Runtime.Refusal`, `isRefusal` and the Nexus `Mesh` adapter are new or mixed-tier public names with no tier; Valance's `./internal` entry is published with its tier only in prose (N7, V7 open). | Nexus `docs/`; Valance `package.json` exports. | Should fix | Copy `docs/manual/stability.md` to each repo with its own rows; mark `Refusal`/`isRefusal` Unreleased. |
| G6 | 13.3 | Port's package README did not carry the `unmount` contract and `Symbol.dispose` that its TSDoc and changelog did. | `packages/port-web/README.md` | Fixed in this pass | None. |
| G7 | 13.3 | The first baseline note said `render` takes `keep`; the option is gone. | `2026-10-09-api-review-baseline.md` | Fixed in this pass (a note on the record) | None. |
| G8 | 7, 11 | Linear memory of the module never shrinks after a burst of renders (362 MB stayed after all renders were released). Not new, but retention makes a burst possible. | Same measurement as G1. | Note | Document; resolves mostly with G1. |
| G9 | 18.1 | No test exercises the realistic failure a host would hit: many renders without release. The memory test releases everything. | `packages/mesh-runtime/test/memory.test.mjs` | Should fix with G1 | Add a test that renders N times without releasing and asserts the module doesn't retain. |

### Checked and fine

Principle 9.1 for Mesh's thrown errors (stable `code`, `cause` kept); 7 for `release()` and `[Symbol.dispose]` (idempotent, documented, tested); 8 for the one-at-a-time call queue (documented); 4.5 for `RenderVersion`; 9.2 for `Refusal` staying a defect while becoming identifiable; 15.4 for Mesh; 10 for hints being static text per code (no input echoed).

### What needs your decision

1. **G1** is the one that matters. Recommended: restore `render()` to keep nothing, keep `update`/`updateChanges` retaining, and let `updateChanges` derive a plain render. I did not change it, because it reverses the option you chose last time.
2. G2: state the "ignore unknown properties" rule (recommended) or version the schema.
3. G3: rename `Refusal.reason` to `code` while it is unreleased (recommended).
4. G4: document that causes are logged (minimum) or log less.
5. G5: add stability pages to Nexus, Valance and Port.

### Third pass: the second pass's gaps closed (G1 to G5, "go for recommended")

- **G1 (done).** `render()` keeps nothing in the module again, as in 0.9.0. `update` and `updateChanges` keep the render they return. `updateChanges` accepts any render: the module derives one it doesn't hold from its own snapshot, under the version it was given. Versions are now the package's to give (a counter in the package, passed to the module), because a version in `render()`'s result made the same inputs give different bytes and broke the native-parity tests. Measured with the earlier loop (2,000 renders of 500 items, never released): nothing retained and 5 MB of memory, against a crash at 2.15 GB. New tests: a no-release loop in the memory test (G9), changes to a plain and to a released render, a derived render under its version in Rust. `Render::with_version` is new Rust API (unreleased).
- **G2 (done).** The `-v1` rule (optional properties may be added without a version change; consumers ignore unknown properties) was already in four schemas; it is now in `render-patch-v1`'s too, and stated once in [API stability](../../manual/stability.md).
- **G3 (done in Nexus).** `Runtime.Refusal` carries `code` (and the type is `RefusalCode`), not `reason`. The rule, written in Nexus's runtime reference and stability page: thrown and defect errors carry `code`; typed failures in the `E` channel carry `_tag`. Valance's defects (V4) follow when it can depend on a Nexus that has `Refusal`; until then they remain message-prefixed, and that inconsistency is open.
- **G4 (done as documentation, the minimum).** Valance's stability page has a "What is logged" section and the startup-work guide points to it: both logs (failed popstate or link, failed start-time command) carry the whole cause, VALANCE does not redact it, and applications keep secrets out of escaping errors or install a redacting logger. Logging less is still open.
- **G5 (done).** Stability pages in Nexus, Valance and Port, each with its own rows. Two Nexus rows are honestly "Unclassified" (`Semantic`, and the event-bus plumbing) because the repository does not say which tier they are in.

Tests after this pass: Mesh runtime 97 and compiler 46 (JS), Rust workspace clean; Nexus 496; Valance 114 and the tracer suite 548; Port 312.

### Still open after the third pass

1. The shutdown design ("waiting for these effects / close anyway") is deferred. Facts gathered: termination does not wait for `run`/`runFork` effects today (documented and pinned by a test); servers are told to stop by SIGTERM with a deadline (30 s Kubernetes, 90 s systemd, 10 s Docker), where only a logged list of blockers and a deadline make sense, and a human prompt belongs to terminals (Ctrl+C twice) and in-page `stop()`.
2. Valance defects are message-prefixed until a Nexus release with `Runtime.Refusal` exists (V4).
3. Logging less than the whole cause in Valance's two logs (G4).
4. Nexus `Semantic` and the event-bus plumbing are "Unclassified" until a tier is chosen.
5. Other Nexus and Valance findings in the tables above that were not taken (N2, N5, N6, N8 to N15, V2, V4 to V17).
6. The browser tests have not run in this environment.

### Fourth pass: documentation drift (after Valance 0.6.0 and Nexus 0.11.0 were released)

Two read-only audits (Nexus; Mesh and Port) compared the docs with the exported APIs, running the examples where they could, and the findings were applied.

- **Nexus (docs only).** Refusal is Stable since 0.11.0 and the release notes say "Released". Resource release is stated as it is: layer and platform resources are released at termination, a command's own `Effect.scoped` resource when the command ends. `State.update` trusts its function; only `create` and `set` validate. `run` rejects with a `FiberFailure`, so `isRefusal` is false on the rejection (use `runFork` and `Cause.dieOption`). `Service.layer`'s generics, `Selector.combine`'s completion, the event bus rules (subscription starts when run, tag uniqueness, unbounded buffering), the architecture examples and decision 16, the README status order, and the roadmap status were corrected or added.
- **Mesh (docs and code).** `updateChanges` takes any render (the README still said otherwise); a released `updateChanges` render fails with `render-gone`; `diff` is the second named exception; `mesh_render_kept` no longer appears; `hint` is in the compiler package's types and a docs example; the NEXUS guide shows the error classes, `updateChanges` and the release pattern. In code: `init` and the browser auto-load now reject with coded errors (`module-unavailable`, `invalid-argument`, `version-mismatch`), and an operation given a foreign `Render` names itself.
- **Port (docs and code).** `[Symbol.dispose]` is a member of `WebPort` (it was optional, so `using port = ...` did not typecheck); the error table has `unsupported-patch`, `unknown-key` and `patch-failed`; the contract table row is fixed; the stability page lists the server entry.
- **`using` needs a runtime with explicit resource management** (a syntax error on Node 22); the docs now say so and show `release()` in a `finally`.

Still open, as code decisions (not documentation):

1. `Runtime.isRefusal` returns false for `Runtime.run`'s rejection, which is a `FiberFailure`. Either `isRefusal` unwraps it or `run` rejects with the `Refusal`.
2. `Selector.combine(...).changes` emits nothing until every input has committed once (affects `Mesh.host` over a combined selector).
3. A bare `Resource.acquire` run through `Runtime.run` does not typecheck, but run untyped it is never released.
4. Examples not run by any test: the two package READMEs' first snippets, three blocks in `using-mesh-from-javascript.md`, and Port's README snippets.

### Fifth pass: the four open code decisions ("go for recommended")

1. **`isRefusal` and `run`'s rejection (Nexus, unreleased).** `Runtime.refusalOf(value)` returns the `Refusal` in a `Refusal`, a `Cause` or `run`'s `FiberFailure`, and `undefined` for anything else. `isRefusal` keeps its meaning (it is a type guard for the value itself). Valance's `isRefusal` will use `refusalOf` once it depends on a Nexus that has it.
2. **`Selector.combine(...).changes` (Nexus, unreleased).** It now emits on every later commit of either input, combined with the other's latest value; it used to wait until both had committed. It is the combined `values` without its first element. A test commits one input, then the other.
3. **A bare `Resource.acquire` run through `Runtime.run`.** The earlier finding was too strong: it is not "never released", it is released when the `Scope` given to `Runtime.make` or `Application.start` closes, not at termination, and it cannot be written without bypassing the types (`Scope` is in its requirements). No code change; the resource page says so.
4. **Examples run by tests.** The Mesh runtime README sample, the compiler README sample and the first three samples of the JavaScript guide (check, slice a span, repair loop is covered by the compiler's guide test) now run as tests, with their imports pointed at the built packages and any other free name a `ReferenceError`; the browser sample can't run outside a browser and the test says why. Port's README samples (draw/update/unmount, server HTML, hydrate, error handling) run in jsdom. The Mesh README sample now takes `snapshot` instead of an invented `{ users, selected }`, and Port's error-handling sample imports `WebRealizationError`.
