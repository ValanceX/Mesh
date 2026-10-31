# MPRX: the space between two interpolations (RF-13, WP-26)

*Design note, 2026-10-16. Written before the change; the "Implementation status" section at the end is filled in after it.*

**The question.** `<text>{first} {last}</text>` renders `AdaLovelace`. The space is a text child that is whitespace only, and the semantic model drops those (MPRX-SPEC §3, "Whitespace in child content"). The Valance orders-app found it writing `#{r.id} {r.customer}`; the workaround was a separator character. One application, one occurrence, but it is not an accident of that application: §9.7 defines the content of an element as "a text run … the concatenation of its literal text and interpolations, in order" and §10 names that run as a *site*, and a run that cannot hold a space between two of its parts is not the run the text describes.

**What other templating languages do** (from their documentation, not tested here). JSX drops whitespace that contains a line break and keeps whitespace that does not (`{a} {b}` reads "a b"). Vue's default (`condense`) removes whitespace between elements when it contains a line break and condenses it to one space when it does not. Svelte keeps a single space between inline content. All three separate *formatting* (the line break and indentation a person lays a template out with) from *content* (a space written on the line).

## Statements

- **W-1 The rule.** A text child of an element's content that is whitespace only, has no line break (`\n` or `\r`), and has an interpolation as the child immediately before it and an interpolation as the child immediately after it, is content: it is kept in the semantic model verbatim, and is part of the text run like any other literal text.
- **W-2 Line breaks stay formatting.** Whitespace-only text that contains a line break is dropped as before, wherever it is. So `{a}` and `{b}` on separate lines read `ab`, as they do today, and a formatter or a person who re-wraps a template between two interpolations changes nothing it renders.
- **W-3 Only between two interpolations.** Whitespace-only text next to an element, a block, a slot, the start or the end of the content is dropped as before. `<b>x</b> <i>y</i>` still renders no space between the two elements; that is a separate question (the inline-element case), with no application asking for it yet, and a stray text child in a column or row is not harmless on every target (the Desktop PORT draws a text node as a line of its own).
- **W-4 Where it applies.** In the semantic IR's lowering of children (`mesh-semantic::lower_element`), the one place whitespace is filtered. The CST and AST keep every text node, as before. Nothing downstream changes: the compiler's text run, the runtime's render tree and the sites are built from the IR.
- **W-5 `{" "}` stays valid.** A string expression with a space still works, and is how a space is written when it must sit at a line break or next to an element.
- **W-6 Compatibility.** A program with no whitespace-only, line-break-free text between two interpolations renders exactly what it did, with the same node identities and handler identifiers. A program with one gains the space it wrote: this is a change of the rendered text, so it is a **minor** version (0.13.0) with a changelog entry under "Changed", not a patch.
- **W-7 Tooling.** `mesh-format` and the language server do not change the space (they touch whitespace between tokens, and this space is content); a test says so. The grammar does not change.
- **W-8 Composite children.** The check that a composite's children have a slot counts an interpolation as a child already, so a kept space between two of them adds nothing to it.
- **W-9 Tests.** `{a} {b}` renders with its space; `{a}` newline `{b}` does not; `{a}  {b}` (two spaces) keeps both, as text is kept verbatim; `<b>{a}</b> <i>{b}</i>` and `{a} <b/>` are unchanged; a run `{a} {b} x` equals `{a} {b}` followed by `x`; and the render tree's text child is one string.
- **W-10 Documentation.** MPRX-SPEC §3 states the rule and the line-break distinction; the manual's templates page says how to write a space (inline, or `{" "}`).

## Implementation status (2026-10-16)

| Statement | Status | Evidence |
|---|---|---|
| W-1, W-4 | Matches: `is_space_between_interpolations` in `mesh-semantic::lower_element`, the one place whitespace is filtered; the AST and CST are unchanged | `crates/mesh-semantic/src/lib.rs` |
| W-2, W-3 | Matches | `crates/mesh-runtime/tests/whitespace.rs`: a line break between two interpolations reads `ab`; whitespace next to an element or an edge is dropped |
| W-5 | Not changed; the string expression was never affected | |
| W-6 | Matches for the programs in the repository (the full workspace suite passes unchanged, including the conformance and snapshot tests); versioning is for the release | `CHANGELOG.md` `Unreleased` |
| W-7 | Matches | `crates/mesh-format/tests/format.rs` `leaves_a_space_between_two_interpolations_alone` |
| W-8 | Not separately tested: an interpolation is a child already, and the kept space sits between two of them | |
| W-9 | Matches, and the new tests fail without the change (three of the five do, as intended; the other two pin what must not change) | `whitespace.rs` |
| W-10 | Matches | `docs/reference/mprx-spec.md` §3, `docs/reference/templates.md` |
