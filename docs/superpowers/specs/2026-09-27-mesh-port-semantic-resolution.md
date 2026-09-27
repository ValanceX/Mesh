# MESH–PORT semantic resolution: values in text positions, and event attribution

**Date:** 2026-09-27
**Status:** proposal for review. Nothing here is implemented, and no source, schema or published document is changed by it.
**Against:** ValanceX/Mesh `a03733e` (v0.5.0 plus one docs commit). PORT's handoff: ValanceX/Port `claude/focused-keller-jf7dt2` at `8be4d10`, `docs/architecture/2026-09-27-mesh-semantic-handoff.md`, `…-value-realization-audit.md` and `…-event-propagation-audit.md`.
**Scope:** the questions V-Q0–V-Q5 and E-Q1–E-Q4 of PORT's handoff. SSR, hydration, HTML serialization, event delegation and mismatch handling are out of scope and stay blocked until this is approved. NEXUS plays no part in any of it.

Every question was checked against the grammar (`grammar/tree-sitter-mprx`), the semantic model (`mesh-semantic`), checking and compilation (`mesh-analysis`, `mesh-compiler`, `template-v1`), `render-v1`, the Rust runtime (`mesh-runtime`), the JavaScript runtime (`@valancex/mesh-runtime`) and the reference host and renderer (`crates/mesh-runtime/tests/common/`, `packages/mesh-runtime/test/`), the tests and fixtures, and the documents (`docs/MPRX-SPEC.md`, `docs/ARCHITECTURE.md`, `docs/manual/*`, `docs/guides/*`, the v0.5 outline). Web DOM behaviour is used as evidence of nothing.

---

## Classification

| Question | Class | In one line |
|---|---|---|
| **V-Q0** semantic values, or values realized for a position? | **2. Implied, not contractual** | Semantic values. MESH has no notion of an output position, and "final" means *evaluated*, but no document says so. |
| **V-Q1** text for a number, boolean or `null` prop | **5. Needs a contract change** | Which text is a small extension of §9.7.7. But only the runtime may make text (rule 9), and its one channel to a renderer, `render-v1`, carries none for props, so it can't be delivered today. |
| **V-Q2** `null` in a text position | **3. Unspecified** | Nothing covers it. MESH keeps `null` and absent distinct, and already gives `null` a text in content. |
| **V-Q3** lists and records in a text position; structural lowering | **2. Implied, not contractual** | §9.7.8's reason ("no text form of one is obviously right") holds for props too, but is written only for content. Structural realization isn't addressed. |
| **V-Q4** a target's own conversion (`DOMString` given a number) | **4. Contradiction** | Rule 9 and I11 forbid anything outside the runtime to "convert a value to text", yet I11 also calls "setting a DOM attribute" from a tree's values drawing. |
| **V-Q5** the guide's "MESH already turned it into text" | **4. Contradiction** | True of text runs, false of props (§9.8.2). |
| **E-Q1** can one interaction trigger several bindings? | **3. Unspecified** | MESH never mentions a user interaction, only "a primitive's event fires". |
| **E-Q2** can a descendant's interaction reach an ancestor's binding? | **2. Implied, not contractual** | Events are declared per component, a binding is one node's, and no relation between two nodes' events exists. So no event propagates, but it's never stated. |
| **E-Q3** which interactions a primitive's event covers | **3. Unspecified** | A manifest event is a name and a payload. MESH has no built-in components (§9.1), so it can't define what any one primitive's event means, and doesn't say who does. |
| **E-Q4** propagation control in MPRX | **2. Implied, not contractual** | A handler is exactly one command invocation (§9.5), and there is nothing to control if no event propagates. |

No question is class 1: MESH answers none of them in so many words.

---

## A. Existing MESH facts

Only what the current code, specification, documents and tests support. References are to `docs/MPRX-SPEC.md` (§), `docs/ARCHITECTURE.md` ("rule n"), `docs/manual/runtime.md` ("runtime manual") and the v0.5 outline (`docs/superpowers/specs/2026-09-25-mesh-v0.5-outline.md`, "I11" etc.).

### Values

- **F1. A prop value is typed by its component's declaration, and checked before it leaves.** At every primitive occurrence the runtime evaluates each written prop and refuses it unless it *fits* the prop's declared type (§9.7.6 item 4; `crates/mesh-runtime/src/render.rs`, `node()`, which reads `valid.component(..).props[..].ty`). So the runtime **has** each prop's declared type at render, and the value it emits always fits it.
- **F2. The emitted value is the boundary value, unchanged.** `boundary::output` maps `null`, booleans, finite numbers (`-0` to `0`), strings, lists and records one to one into JSON (`src/boundary.rs`). §9.8.2's "Out" column: props go out "as is".
- **F3. render-v1 carries a component name, and per prop a name and a value.** No declared type, no requiredness, no target slot (`schemas/render-v1.schema.json`, `$defs/node`). I13 lists exactly what a tree holds, and a type or slot isn't among them.
- **F4. MESH defines text for values, but applies it only to content.** §9.7.7: a string is itself, a boolean `true`/`false`, `null` is `null`, absent is the empty string, a finite number by §9.7.7.1. The runtime applies it only to interpolations (`render.rs`, `text()`), and a text run reaches the renderer as a string.
- **F5. Lists and records have no text, by decision.** §9.7.8: `content-not-text` at check time, `runtime-content-not-text` for an `any` at run time, because "no text form of one is obviously right".
- **F6. Only the runtime may make text.** Rule 9: "Nothing outside it evaluates an expression, turns a value into text, coerces or normalizes a value …". I11 says the same ("no code may … convert a value to text, coerce a value between kinds").
- **F7. Renderers depend on the tree's types, never on the runtime.** Rule 13; the rendering guide: "A renderer imports only the tree's types, never the runtime". `@valancex/mesh-runtime` exports `render`, `dispatch`, `init` and `version`, but no text conversion. The Rust crate exports `number_to_text`.
- **F8. MPRX can't make text in a prop.** §9.4: `+` is numeric only, no concatenation, no built-in members or functions. A prop gets a string only when a literal or a host value already is one.
- **F9. MESH has no notion of an output position, and assumes no target.** "MESH stays renderer-independent. It never assumes a DOM, a canvas, or a particular device" (ARCHITECTURE); rules 2 and 14.
- **F10. `render-v1` may grow.** Its schema: "A later MESH may add properties without changing `version`; removing one or changing its meaning is a new version. Renderers must ignore properties they don't know." Rule 15: optional information "improves realization but is never needed for correctness".
- **F11. "Final" is described, not defined.** Runtime manual: a renderer "draws each node's component with its props … as given. It computes nothing: it doesn't format numbers, supply a missing prop, or convert values. Values are final". Nowhere is "as given" defined for a target that holds only text.
- **F12. The reference renderers print props as JSON, and say so.** `tests/common/renderer.rs`: "prop values are printed as the JSON they are"; the guide's renderer uses `JSON.stringify`. Both are test and teaching printouts, and I11 allows code outside the runtime to "encode and decode [values] losslessly".

### Events

- **F13. A binding is one element's declaration.** §4: "this element emits this event, handle it with this expression". §9.1: `on.` bindings are the instance's events, checked against its own component.
- **F14. Events are per component.** A manifest event is `{}` or `{ "payload": T }` inside one component (`docs/manual/manifest.md`, "Events"). Props, events, commands and scope names are separate namespaces per component (§9.1). The slice's `avatar.click` (payload `Press`) and `button.click` (no payload) are unrelated declarations with one name.
- **F15. MESH has no built-in components.** §9.1: "There are no built-in elements." Nothing in MESH says what any particular component, or any of its events, does.
- **F16. One handler identifier names one handler at one node,** from the program identity, the node's key and the event's name (runtime manual). Dispatch maps one identifier and payload to one intent (`src/dispatch.rs`).
- **F17. The payload is the node's own event's.** §9.5: `$event`'s type is "the payload of the handled event, as declared by the component of the element the `on.` sits on"; §9.8.4 item 3 checks it against that event.
- **F18. The lifecycle speaks of emission, singular.** Runtime manual step 3: "A primitive's event fires. The renderer reports the handler identifier from the tree it drew, and a payload." The guide: "When the user does something, report the node's handler identifier for that event".
- **F19. Composites have no events, and vanish from the tree.** `assembly-composite-event` (templates manual rule 6); composites are expanded, so tree ancestry is between primitive nodes only, whichever template wrote each binding. A handler inside a composite invokes that composite's own command.
- **F20. A handler is exactly one command invocation** (§9.5: `handler-not-command`, `command-outside-handler`). There is no syntax around it.
- **F21. `on.click` is interaction intent, not a DOM event** (ARCHITECTURE, "Direction").
- **F22. No MESH program that is checked or run nests bindings.** The only MPRX with a bound node inside a bound node is `grammar/tree-sitter-mprx/test/captures/events.mprx`, a highlighting sample. The runtime's test host fires an event by picking one node that binds it and dispatching once (`tests/common/host.rs`). One node binding two events does occur: `tests/programs/greeting/view.mprx` (`probe` with `on.tap` and `on.click`) and `examples/fixtures/check/pass/resolution.mprx` (`button` with `on.click` and `on.press`), each dispatched separately.
- **F23. Propagation is never mentioned.** "bubble", "propagate", "capture" and `stopPropagation` don't occur in any MESH document, test or source in an event sense (the only hits are about diagnostics).

---

## B. PORT's findings

From the handoff and its two audits. PORT's evidence agrees with section A wherever the two overlap.

- **B-1.** render-v1 has been enough for everything PORT built: drawing, keyed updates, event reporting, and program continuity with no program identity in the tree.
- **B-2.** render-v1 carries neither a prop's declared type nor its target slot (= F3). So "values already realized for their output position" can't be implemented from the current contract.
- **B-3.** Strings reach every Web position unchanged, and booleans can be realized by presence. A number, a boolean as text, `null`, a list or a record in an attribute, and so in server HTML, has no text PORT may use. The Web PORT refuses each with `unrealizable-value`.
- **B-4.** MESH's number text is available in Rust but not to a JavaScript renderer (= F7), and a platform's `ToString` isn't MESH's text in the last digit (§9.7.7.1).
- **B-5.** Two inconsistencies in MESH's documents: the guide says numbers arrive as text (VC1), and MESH's reference renderers print props with a formatter MESH doesn't call authoritative (VC2).
- **B-6.** Today's Web PORT bubbles as the DOM does, across differently named events realized as one DOM event type. It is pinned as non-contractual characterization.
- **B-7.** PORT's reading of the event evidence: "local binding, nearest bound node" (B1) is best supported; bubbling has no source but the DOM. PORT rejected failing closed on nested bindings, and adopts no answer.
- **B-8.** SSR is blocked on the value question only; hydration's interactive half also on the event question. Nothing else is missing.

---

## C. What MESH has already determined

Existing semantics, stated as MESH would state them. Nothing here is new; section E marks what is.

1. **render-v1 carries semantic values** (V-Q0). A prop value is the evaluated value of its expression, in the boundary data model, checked against the declared type of its prop (F1, F2). "Final" means that evaluation is complete: nothing is left to compute from MPRX. It can't mean "realized for its output position", because MESH has no output positions (F9) and nothing in the tree could say which one (F3). Realization is PORT's.
2. **The text of a value is MESH's, and only the runtime produces it** (F4, F6). A renderer must never make MESH text itself, including with a platform's conversion.
3. **Lists and records have no text** (F5). MESH decided this for content, for a reason that doesn't depend on content.
4. **Absent is omission, never a default, and never `null`** (§9.2, §9.8.2, runtime manual).
5. **A binding belongs to exactly one node, and handles that node's event** (F13, F16, F17). One reported handler identifier is one dispatch and one intent.
6. **No event of one node is an event of another** (F14, F19). There's no MESH relation between two nodes' events, even with one name, and no composite events that could forward one. So MESH has no propagation.
7. **MPRX can't express propagation control** (F20), consistent with 6.
8. **MESH defines no particular component's behaviour** (F15). What a given primitive does, including what its events mean, isn't MPRX semantics.

---

## D. Genuinely unresolved questions

What remains once section C is taken as settled:

- **U1. Text for a scalar prop in a text position** (V-Q1, V-Q2). When a PORT realizes a number, boolean or `null` prop where its target holds only text, what text is it, and how does it reach the PORT when only the runtime may produce it? `null` is the sharpest case, since omitting it would make it absent.
- **U2. What "as given" means for a prop** (V-Q3, V-Q4). Which realizations of a value are drawing, and which are conversion? Is a target's own coercion allowed? Can a list or record be realized structurally?
- **U3. How many bindings one interaction triggers, and which** (E-Q1, with E-Q2's "nearest" variant). MESH defines emissions and bindings, but not how a target interaction becomes an emission when bound nodes are nested, or when one node binds several events.
- **U4. Who defines what a primitive's event covers** (E-Q3). MESH can't (F15); it must at least say whose question it is, and which part is its own.
- **U5. The contradictions** (V-Q4, V-Q5, and two found in this audit): see "Contradictions" below.

### Contradictions

Two confirm PORT's, and two are new.

- **K1 (PORT's VC1, confirmed).** `docs/guides/rendering-mesh-output.md`: "Don't format a number (MESH already turned it into text, by its own rule…)". This holds for text runs. A number prop reaches the renderer as a number (§9.8.2).
- **K2 (new; it underlies V-Q4).** I11 in the v0.5 outline says, in one list, that no code outside the runtime may "convert a value to text [or] coerce a value between kinds", **and** that "a renderer mapping a render tree's strings and values onto its target (setting a DOM attribute, drawing text) is drawing, not evaluation". Setting a text-only attribute to a non-string value *is* converting it to text, by the platform. The example is also Web-specific, in a MESH invariant (rule 14). `ARCHITECTURE.md` rule 9 keeps only the first half.
- **K3 (PORT's VC2, reclassified).** The reference renderers' JSON printing is lossless encoding, which I11 allows, not MESH text. It's not a contradiction, but the guide presents its printout as "a renderer, end to end" and draws props as `size="sm"`, which reads as a model for real text targets. It needs a label, not a rule.
- **K4 (new).** The runtime manual's example tree gives `avatar` the props `"src": null` and `"size": 48`. Against the slice's manifest (`src: string?`, `size?: string`) neither can occur: `null` doesn't fit `string?`, and a number doesn't fit `string`. The example is described as illustrative only for its keys and identifiers, so it silently implies a different model, with a number prop that no renderer could put in a text position.

---

## E. Proposed semantic resolution

The smallest set of MESH-level rules that closes U1–U5. Each is target-independent, and none adds MPRX syntax.

### E1. Realization of a prop value (U2; V-Q0, V-Q3, V-Q4)

**Meaning.** A renderer realizes each prop value in one of exactly two ways:

1. **Natively.** The value goes into a target slot that holds a value of the same kind, and holds it *exactly*: a string where the target holds a string; a boolean where it holds a boolean state (a flag, a switch, a presence); a number where it holds a number that represents this binary64 value exactly; `null` where it holds a null; a list or record where it holds a structure of the same shape, each element or field realized natively by these same rules.
2. **As its text** (E2), where the target holds only text.

Nothing else is realization. A target's own coercion (a string slot that stringifies a number, an integer slot that truncates `1.5`), JSON or any other encoding presented as the value, and any lossy mapping are conversions, and a renderer must not use them. When a primitive's realization has no conforming slot for a value, the renderer reports a realization failure and substitutes nothing, as PORT's `unrealizable-value` already does. Absent is omission, as today.

A list or record has no text (E2), so it can only be realized natively. A per-primitive structural realization, such as a `list<string>` of class names into a target's native token list, is native realization, provided each element is realized exactly; an element the target structure can't hold makes the whole value unrealizable. Whatever textual serialization the target later applies to its own native structure is the target's encoding, not MESH text.

Which slot a primitive's prop goes to, and so whether the text path is ever taken, is the primitive's realization and PORT's to choose (C8). MESH decides only what may happen to the value once the slot is chosen.

This answers **V-Q0** (semantic values, realized by the PORT under these two rules), **V-Q3** (never as text; natively, structurally, or not at all) and **V-Q4** (a target's coercion is forbidden formatting: a number goes natively into an exact numeric slot, or as MESH text into a text slot).

**Declared types aren't needed.** Both rules depend only on the value's kind, which the tree carries, and the runtime has already checked the value against its declared type (F1). So neither the declared type nor the slot needs to enter render-v1, and primitive declarations take no further part in realization.

**Effects.** MPRX, the compiler and the runtime: none. render-v1: none; its descriptions of "final" and "as given" change. Reference hosts: none. Future PORTs: one rule for every target: exact native slots, or MESH text. Compatibility: nothing that conforms today stops conforming. It makes explicit that the Web PORT's refusals are correct, and that any `setAttribute(name, number)` or `String(x)` path is not.

### E2. The text of a value applies to props (U1; V-Q1, V-Q2)

**Meaning.** §9.7.7's text becomes **the** text of a value, wherever MESH or a realization presents a value as text, props included:

| Value | Its text |
|---|---|
| string | itself |
| boolean | `true` or `false` |
| `null` | `null` |
| finite number | by §9.7.7.1 |
| list, record | none (E1: native only, or unrealizable) |
| absent prop | none: the prop is omitted (unchanged; absent's empty text stays specific to content) |

- So **`null` in a text position is the text `null`** (V-Q2). It is neither omission, which would make it absent, contrary to §9.2, nor unrealizable. Where a primitive's realization gives `null` a native meaning instead (a nullable slot), E1's native rule applies.
- Text conversion stays entirely MESH's (F6). A PORT decides *that* a prop is presented as text, by choosing a text slot for it. It never decides *what* the text is.

**Rejected alternatives.** JSON: it is a data encoding, not a presented text (`"Ada"` is quoted), and choosing it because it's available would be a second, accidental text rule. Omission for `null`: it erases a distinction MESH keeps deliberately. A text per primitive or per slot: that would make text depend on the target, contrary to F9.

**Effects.** MPRX and the compiler: none. Spec: §9.7.7 is restated as the text of a value, and §9.8.2's "Out" column gains a prop's text. The runtime computes the text (E3).

### E3. The runtime delivers a scalar prop's text in render-v1 (U1, delivery)

**Meaning.** A text must reach a renderer that may not make it (F6) and depends only on the tree (F7). So the runtime puts it in the tree. Each node gains an **additive** member, provisionally `propText`, mapping each prop whose value is a **number, boolean or `null`** to that value's text (E2):

```json
{ "type": "node", "key": "k…", "component": "meter",
  "props": { "label": "Load", "value": 0.30000000000000004, "busy": false },
  "propText": { "value": "0.30000000000000004", "busy": "false" },
  "events": {}, "children": [] }
```

- Strings aren't repeated, because a string's text is itself. Lists and records have none. An absent prop has neither entry.
- `props` keeps its meaning. `propText` is derived: a pure function of the prop values, computed by the one evaluator at render.
- A renderer that ignores `propText` stays correct, because under E1 it refuses what it can't realize (rule 15). One that uses it can realize every scalar in a text position, and a server and a browser realization get byte-identical text from the same tree, which is what hydration will compare.

**Why this belongs in the contract, and isn't just something PORT wants.** The text of a value is already MESH semantics (§9.7.7), and only the runtime may produce it (rule 9). A tree is the runtime's only channel to a renderer (rule 13). So if a prop's text exists for anyone, it has to be in the tree. That is the test ARCHITECTURE sets ("did the semantic contract change?"): yes, E2 extends it, and render-v1 carries the result.

**Rejected alternatives.**
- *Export a text function from `@valancex/mesh-runtime` for renderers to call.* It breaks F7 and rule 13, since every renderer would then depend on the runtime and its WebAssembly. A non-JavaScript PORT would have to link the runtime or reimplement §9.7.7.1, and a reimplementation is the second evaluator rule 9 forbids.
- *Put the declared type, or a "text slot" marker, in render-v1.* Neither is needed (E1), and a slot marker is target knowledge MESH doesn't have (F9).
- *Replace number props with their text.* That changes the meaning of an existing property, which is a new format version, and it loses the value for native slots.
- *Require every prop a target shows as text to be declared `string`.* It still leaves a number-typed prop that a primitive legitimately shows as text, such as a meter's value, and MPRX can't build a string in a prop anyway (F8).

**Effects.**
- MPRX, the compiler, `template-v1`: none.
- render-v1: one optional property on `$defs/node`. Under the schema's own promise (F10) that is not a version change. The schema file changes, because `$defs/node` has `additionalProperties: false`, so a consumer validating new trees against the *old* schema would reject them. The release notes must say so.
- Runtime: `render.rs` computes the text beside each non-string scalar prop, reusing `number_to_text` and §9.7.7. The output check (§9.7.6 item 6) already guarantees a finite, non-`-0` number. Native and WebAssembly outputs stay identical; the JS types gain `propText?`.
- Reference hosts: the reference renderer and its committed `.html` snapshots may keep printing JSON (K3). The parity and slice fixtures change, since trees gain a member.
- Future PORTs: every PORT, in any language, gets MESH's text from the tree. None needs MESH code.
- Compatibility: additive and non-breaking for renderers that ignore unknown properties, as render-v1 requires. Tree size grows by one short string per non-string scalar prop.

### E4. Events are emissions; one interaction is attributed to at most one binding (U3; E-Q1, E-Q2, E-Q4)

**Meaning.**

1. **An event is an emission by one primitive occurrence** (formalizes C5). A binding `on.e` on node N handles `e` emitted by N and nothing else. Each emission yields at most one report (N's handler for `e`, if bound), one dispatch and one intent.
2. **No propagation** (formalizes C6). An emission by one node is never an emission by another. Events of different nodes are unrelated, whatever their names; there's no bubbling and no capture.
3. **Attribution.** When a target interaction occurs, its **target node** is the innermost render-tree node the interaction is on, with a text run counting as its parent node's. From the target node towards the root, the **first node that binds one of its events the interaction realizes** (E5) emits that event, and **no other node emits for that interaction**. A node that binds no such event is transparent: the interaction passes it by. Composite boundaries play no part, since composites are expanded (F19).
4. **One event per node per interaction.** A primitive's realization maps any one interaction to at most one of that primitive's events. So a node binding `tap` and `click` never has both fire from one gesture; if a realization maps both to one gesture, it doesn't conform.

So one interaction causes **at most one intent**. For

```text
<parent on.click={parent()}>
  <child on.click={child()} />
</parent>
```

an interaction on `child` that `child`'s `click` covers invokes `child()` only. An interaction on `parent` outside `child` invokes `parent()`. If `child` bound nothing, an interaction on it would reach `parent`, if it realizes `parent`'s `click`.

**Answers.** **E-Q1:** no. One interaction triggers at most one binding, on nested nodes and on one node alike. **E-Q2:** an ancestor's binding is triggered only when no nearer node on the path binds an event the interaction realizes, and then as that ancestor's own event, never as a relation between two nodes' events (none exists). Yes, there is a nearest binding; attribution follows render-tree ancestry, not event names. **E-Q4:** there's nothing to stop, so MPRX needs no syntax. An application that wants one gesture to do two things writes one command that does both; MESH records intent, and the host decides what it does (§6).

**Why this rule.** It is the only reading consistent with every existing statement (F13–F22): one node, one handler, one dispatch, one intent. It keeps `on.click` interaction *intent* (F21): one user act states one intent. It is decidable from the tree alone, because ancestry and bindings are both in render-v1. And it asks nothing of MPRX. Bubbling would need a cross-component relation between events that MESH deliberately lacks (F14), a payload rule for an ancestor receiving a descendant's interaction, and new syntax to stop it; nothing in MESH supports it (F23). "Target only" makes a primitive with content unusable. A declaration-based variant, where the innermost node that *declares* a matching event absorbs the interaction even when unbound, would need the manifest in the renderer, and would let an unbound avatar silently swallow a click meant for its card.

**Effects.** MPRX, the compiler, render-v1 and the runtime: none. Dispatch is per identifier and needs no change. Spec: a new §9.9, "Events", holds rules 1–4; the runtime manual's lifecycle step 3 refers to it. Reference hosts: the test host already dispatches once per event (F22). MESH adds language-neutral conformance vectors: a tree, an interaction target node, which of each node's events the interaction realizes, and the expected single handler identifier or none. They include a nested case, a transparent unbound node, a composite boundary, and one node with two events. Future PORTs: every target implements one hit-test-then-walk rule. On the Web this means not relying on DOM bubbling; how is PORT's business. Compatibility: no MESH program changes meaning, because none nests bindings in a checked or run corpus (F22). The Web PORT's non-contractual bubbling (B-6) becomes non-conforming.

### E5. What a primitive's event covers (U4; E-Q3)

**Meaning.** Two parts, owned by different layers:

- **Containment is MESH's.** An interaction on a node's content, meaning its descendants and text runs, is an interaction *on* that node for E4's walk. A click on a button's label reaches the button by structure, not by a per-primitive rule.
- **Which interactions realize a given event of a given primitive is that primitive's definition**, not MPRX's: MESH has no built-in components (F15), and a manifest declares only an event's name and payload. Each PORT realizes that definition for its target. For one primitive, the definition must be the same on every target (the same meaning, however it's realized), and it can't depend on anything outside the node: its bindings, its ancestors or its siblings. It must also satisfy E4 rule 4.

So E-Q3 is answered as "left to the primitive, under MESH's generic rules". That isn't PORT inventing MESH semantics: it is PORT defining its own primitives, which MESH never had. MESH's own share, how nested nodes and several bindings combine, is E4.

**Effects.** Spec: §9.9 states the split. PORT: its realization tables become the documented definition of each primitive's events (PORT's work, later). MPRX, compiler, render-v1, runtime: none. Target capability descriptions (ARCHITECTURE, "Targets describe themselves") might one day carry these definitions, but nothing needs them now.

### E6. Corrections (U5)

- **K1:** the guide says numbers arrive as text **in text runs**. A number prop arrives as a number, and its text, when a realization needs one, is in `propText` (E3).
- **K2:** `ARCHITECTURE.md` (rule 9's neighbourhood) and the runtime manual state E1. Drawing is native realization, or MESH's text in a text slot. A platform's conversion isn't drawing. The v0.5 outline is a historical record and isn't rewritten; the new text supersedes its I11 example.
- **K3:** the reference renderer and the guide's example are labelled as printouts that encode props as JSON for comparison, not a realization.
- **K4:** the runtime manual's example tree is made consistent with a model where it can occur.

Documentation only. No behaviour changes.

---

## Answers to the brief, by value and position

"Content" is a text run. "Native slot" and "text slot" are E1's two ways, whatever target slot a primitive's realization picks. On the Web, a DOM property is one or the other depending on its type, and an attribute or SSR HTML is a text slot, except boolean presence, which is a native boolean state.

| Value | Content | Native slot | Text slot (attribute, SSR) |
|---|---|---|---|
| string | itself (§9.7.7) | as is | itself |
| number | §9.7.7.1, already a string | the exact binary64 value, or unrealizable | `propText` (§9.7.7.1) |
| boolean | `true`/`false` | the boolean state (presence included) | `propText`: `true`/`false` |
| `null` | `null` | a null, where the slot has one | `propText`: `null` |
| list | check-time or evaluation error (§9.7.8) | a same-shaped structure, element by element, or unrealizable | unrealizable |
| record | as for list | as for list | unrealizable |
| absent | empty text | omitted | omitted |

- **Valid?** Every cell except "unrealizable" and the content errors.
- **Semantic representation:** the boundary value (§9.8.1), unchanged.
- **Is text conversion MESH semantics?** Yes, for every scalar (E2).
- **Who converts?** The runtime: for content already, and for props via `propText` (E3).
- **Is render-v1 sufficient?** For native realization, yes. For scalars in text slots, no.
- **Must render-v1 change?** Yes: one additive, optional property (E3), which isn't a version change.
- **Do declarations take part in realization?** No, beyond the check the runtime already makes (E1).

---

## F. Required implementation work, after approval

In MESH only. PORT and NEXUS are untouched, and SSR and hydration stay blocked until this lands and PORT resumes them.

1. **Specification** (`docs/MPRX-SPEC.md`): restate §9.7.7 as the text of a value; add a prop's text to §9.8.2's "Out" column; add §9.9, "Realization and events", with E1, E4 and E5; add a revision-history entry. No grammar change.
2. **render-v1** (`schemas/render-v1.schema.json`): an optional `propText` on `$defs/node`, with a description; update "Values are final" to E1's wording. The schema tests and the documents' example trees are updated.
3. **Runtime** (`crates/mesh-runtime`): emit `propText` for number, boolean and `null` props (`render.rs`, `tree.rs`, `Node`); tests that each kind's text equals §9.7.7's, reusing the number-to-text table, and that strings, lists, records and absent props get no entry.
4. **JavaScript** (`@valancex/mesh-runtime`): `RenderNode.propText?` in `types.ts`, plus native–WebAssembly parity covering it. No new exports.
5. **Reference host and renderer:** update fixtures for the new member; label the printouts (K3); add E4's conformance vectors as JSON under `examples/`, with a check that each vector's tree is a real render of its program.
6. **Documents:** runtime manual ("What renderers and hosts must do", the lifecycle, the example tree, K4); rendering guide (K1, E1, E4, E5); `ARCHITECTURE.md` (K2); release notes stating the schema addition.
7. **Release:** a minor version. Nothing a conforming renderer or host does today breaks.

Not in this work: SSR, hydration, HTML serialization, event delegation, mismatch handling, any PORT or NEXUS change, and MPRX syntax. That includes text construction in props (F8), a real language limitation that none of the above depends on.
