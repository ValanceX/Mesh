# Design north star (locked)

This repo is part of the ValanceX ecosystem (Valance, Nexus, Mesh, Port). Every public API here is held to one design north star: **progressive disclosure**. Canonical text: `docs/DESIGN_NORTH_STAR.md` in the Valance repo (https://github.com/ValanceX/Valance). Read it before adding or changing a public API, and name the tier of the change.

**Principles**
1. Minimize *extraneous* cognitive load (boilerplate, manual loops, config mechanics) so authors spend effort on their domain.
2. Hide mechanism (reconciliation, event delegation, graph traversal, batching) behind an explicit boundary; a test should enforce it.
3. Intention-revealing names, signatures and types: say *what*, in domain terms, not *how*.
4. Commands (change state) and queries (do not) are separate; prefer side-effect-free functions and explicit invariants.
5. Match the author's mental model: `UI = f(state)`, one state, commands as the only mutation.

**Three tiers.** Tier 1 is minimal, zero-config, obvious on first inspection. Tier 2 is opt-in tuning (selectors, comparators, schedulers), revealed only when needed, with a default that is correct without it. Tier 3 is an explicit, constrained escape hatch to the platform that cannot corrupt framework invariants. A higher tier never requires knowing a lower one. Mechanism sits behind the architectural boundary, below all three.

**Anti-patterns to reject**
- *Eager configuration:* mandatory parameters for a basic use. Default everything non-critical.
- *Smart UI / leaky mechanism:* callers triggering lifecycle, flush, update or release steps. Encapsulate them.
- *Entity trap:* internal entities or raw runtime structures on a public contract. Expose view models or field-masked DTOs.
- *Architecture sinkhole:* values threaded through components that do not use them. Provide an injection or inherited-scope channel (in MPRX it must stay explicit and type-checked, never implicit).

**Working rules**
- A change that cannot be placed in a tier, or that matches an anti-pattern, is a finding for the owner: report it, do not ship it quietly.
- If this and another document disagree, record the disagreement; do not resolve it silently.
- Score a public-API change with the Valance `docs/NORTH_STAR_SCORECARD.md` (0 to 100, 19 audits): no touched audit may regress, no new Fail ships, and the change states the audits touched with before and after. The current baseline is Valance `docs/north-star-validation-2026-10-19.md`.
- Related: the API Design Principles v1.0 (Mesh `docs/standards/api-design-principles.md`) are the detailed rules.
