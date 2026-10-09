# API Design Principles

Version: 1.0 (status: proposed standard). Scope: public APIs for libraries, frameworks, SDKs, services and platforms. Adopted for the ValanceX ecosystem (Mesh, Port, Nexus, Valance) on 2026-10-09; this is the standard as given, unedited, and the reviews that apply it are in [the review](../superpowers/specs/2026-10-09-api-design-review.md).

## 1. Purpose and design philosophy

A good API enables users to accomplish their goals without requiring unnecessary knowledge of its implementation.

API quality has four dimensions:

1. Understandability: Users can discover and understand the concepts they need.
2. Predictability: Users can correctly anticipate behavior, side effects, and failures.
3. Composability: Capabilities work together without unnecessary coordination or translation.
4. Evolvability: The API can improve without imposing unreasonable costs on existing users.

### 1.1 Core principle

Minimize the concepts, decisions, and coordination a user must manage to accomplish a task, without sacrificing correctness, necessary control, or a clear path to advanced capabilities.

### 1.2 Design hierarchy

When evaluating competing designs, prioritize:

1. Correctness and safety.
2. A coherent, understandable public model.
3. Predictable behavior and composability.
4. Discoverability and ease of use.
5. Flexibility and extensibility.
6. Implementation convenience.

Implementation convenience MUST NOT be the primary reason for exposing internal machinery to users.

### 1.3 Requirement levels

- MUST: Required unless an explicit, documented exception is justified.
- SHOULD: Recommended by default; deviations require a clear rationale.
- MAY: Optional when appropriate to the API's context.

These levels distinguish necessary guarantees from heuristics. A design principle MUST NOT be applied mechanically when doing so would make the API less understandable or less correct.

---

## Part I — Conceptual Design

### 2. Conceptual simplicity

The public API MUST present a coherent model of the problem domain.

- MUST minimize the concepts required to accomplish common tasks.
- MUST give each public concept a clear purpose and responsibility.
- MUST avoid exposing internal subsystems, intermediate representations, and implementation stages unless users genuinely need them.
- SHOULD organize capabilities around user goals rather than internal package boundaries.
- SHOULD make the common path obvious without requiring users to understand advanced capabilities.
- SHOULD reuse existing concepts instead of introducing new abstractions for minor variations.
- MUST NOT expose an abstraction merely because it mirrors the internal architecture.
- MUST NOT require users to coordinate internal steps that the framework can safely orchestrate itself.

#### 2.1 Progressive disclosure

The API SHOULD support three levels of engagement:

1. Basic use: Accomplish a common task with minimal concepts and configuration.
2. Advanced use: Customize behavior through explicit, well-defined options.
3. Deep control: Access lower-level capabilities when the use case genuinely requires them.

Advanced capabilities MUST NOT make the basic path harder to discover.

#### 2.2 One coherent mental model

- MUST use consistent concepts and relationships across related APIs.
- MUST make ownership, data flow, and responsibility understandable.
- MUST avoid introducing different abstractions for the same underlying public concept without a meaningful distinction.
- SHOULD allow users to predict unfamiliar operations from concepts they already understand.
- MUST document important conceptual exceptions rather than relying on surprising behavior.

### 3. Naming and identifiers

#### 3.1 Naming conventions

- MUST follow the host language's established naming conventions.
- MUST use a consistent convention for each member category.

Typical conventions include: types, classes, interfaces and structs in `PascalCase`; functions, methods and variables in `camelCase` in TypeScript and JavaScript; constants and enum members following established platform conventions; private implementation details using language-level privacy or the established private naming convention. These are language-dependent conventions, not universal spelling rules.

#### 3.2 Vocabulary consistency

- MUST use one preferred term for one public concept.
- MUST NOT use synonyms interchangeably when they imply different behavior.
- SHOULD prefer established domain terminology over invented terminology.
- MUST distinguish concepts that differ in ownership, lifecycle, or semantics.
- MUST preserve established public vocabulary unless a deliberate migration is justified.

#### 3.3 Verb and noun conventions

- Actions SHOULD use verb-noun naming: `parsePayload()`, `fetchUser()`.
- Boolean queries SHOULD use `is`, `has`, `can`, or `should` where appropriate: `isValid()`, `hasPermission()`.
- Transformations SHOULD use `to` plus the target representation: `toJson()`.
- Domain entities MUST use noun or noun-phrase names: `HttpRequest`, `UserSession`.
- Process names MUST NOT be used for entity types unless the type genuinely represents a process.

#### 3.4 Symmetric operations

Paired operations SHOULD use established, semantically appropriate pairs: add / remove, get / set, create / destroy or delete, start / stop, enable / disable, open / close.

The pairs MUST represent genuinely corresponding operations. Naming symmetry MUST NOT imply guarantees that do not exist. For example, `remove()` and `delete()` MAY both exist if removing an association differs from permanently deleting a resource.

#### 3.5 Name behavior, not implementation

- Names MUST accurately describe observable behavior.
- Names SHOULD communicate the operation's domain and target.
- Names MUST NOT imply purity, synchrony, safety, or reversibility when those properties are not guaranteed.
- Operations with materially different semantics SHOULD have distinguishable names.

---

## Part II — Signatures and Type Contracts

### 4. Function and method signatures

#### 4.1 Parameter design

- Signatures SHOULD remain short and easy to scan.
- More than three positional parameters SHOULD trigger a design review, not an automatic rejection.
- Four or more inputs SHOULD be grouped into a configuration object when they form a meaningful group of related settings.
- Configuration objects MUST NOT be introduced merely to satisfy a numerical limit.
- Required inputs MUST be distinguishable from optional configuration.
- Optional configuration SHOULD appear after required inputs.
- Callbacks and execution context SHOULD follow a consistent, documented ordering convention.

#### 4.2 Boolean arguments

- MUST NOT use positional Boolean arguments when their meaning is ambiguous.
- SHOULD prefer named options for independent configuration choices.
- SHOULD expose separate methods when the choices represent genuinely different operations.
- MUST NOT create duplicate methods for trivial variations when a named option communicates the distinction more clearly.

Prefer `render({ mode: "interactive" })` over `render(true, false)`.

#### 4.3 Required and optional inputs

- Required inputs MUST be explicit.
- Optional inputs MUST have documented defaults or explicit absence semantics.
- Defaults MUST be deterministic and unsurprising.
- Missing values, explicit null values, and invalid values MUST be distinguishable where their meanings differ.
- Public APIs MUST NOT silently accept invalid values merely to avoid reporting errors.

#### 4.4 Return types

- Every operation MUST have a documented, predictable return contract.
- MUST NOT mix unrelated return types to represent success and failure implicitly.
- SHOULD use explicit result types, option types, exceptions, or another consistent error mechanism appropriate to the language.
- Return values MUST communicate whether an operation has completed, been scheduled, or merely initiated.
- MUST NOT return success-shaped values when the promised operation has failed.
- MUST document whether returned objects are snapshots, live views, references, or handles.

#### 4.5 Type safety

- SHOULD use types to make invalid states unrepresentable where practical.
- MUST encode important constraints in types when the host language supports them effectively.
- MUST distinguish identifiers and values that have different semantic roles.
- SHOULD use discriminated unions or equivalent constructs for finite state variations.
- MUST keep compile-time guarantees aligned with runtime behavior.
- SHOULD preserve useful type inference for common use cases.
- MUST NOT force users into routine type assertions or unsafe casts to use ordinary public functionality.
- SHOULD avoid unnecessarily complex public generic types.

Types MUST improve the user's understanding and correctness, not merely demonstrate the sophistication of the implementation.

---

## Part III — State, Mutation, and Dependencies

### 5. State and side effects

#### 5.1 Immutability by default

- Transformations SHOULD return new values instead of unexpectedly modifying caller-owned values.
- APIs SHOULD prefer immutable data where it simplifies reasoning and sharing.
- Mutating operations MUST make their effects clear through their contracts.
- In-place operations SHOULD use explicit naming when that distinction matters.
- MUST document whether an operation modifies its receiver, its arguments, or external state.

Mutation is permitted when it is appropriate to the resource model. The requirement is predictable mutation, not immutability at all costs.

#### 5.2 Side-effect transparency

- Public APIs MUST document material side effects.
- Read-only operations SHOULD be free of observable side effects.
- Operations presented as calculations MUST NOT silently perform unrelated I/O or modify unrelated state.
- MUST NOT modify global state, environment variables, process-level handlers, or unrelated singletons without an explicit contract.
- Global initialization and registration MUST be explicit when they materially affect application behavior.
- Hidden caching MAY be used when it preserves documented semantics and does not create surprising observable behavior.

#### 5.3 Idempotency

- Read operations SHOULD be repeatable without changing the system.
- State-setting operations SHOULD be idempotent when their semantics permit it.
- Repeated calls to `set(value)` SHOULD produce the same resulting state as one call.
- Creation, increment, append, publish, and other inherently cumulative operations MUST NOT be assumed idempotent.
- APIs involving retries MUST document whether duplicate execution is safe.

#### 5.4 Dependency management

- Dependencies that require substitution, configuration, or deterministic testing SHOULD be injectable.
- APIs SHOULD provide safe defaults for ordinary dependencies when doing so reduces unnecessary setup.
- MUST NOT instantiate hidden, unconfigurable dependencies when users reasonably need control over them.
- Time, randomness, networking, storage, and filesystem access MUST have testable boundaries when they materially affect behavior.
- Dependency injection MUST NOT become an unnecessary prerequisite for basic use.

---

## Part IV — Composition and Configuration

### 6. Composition and ownership

#### 6.1 Composable capabilities

- Related operations MUST fit together through understandable inputs and outputs.
- SHOULD avoid unnecessary conversion between public concepts.
- MUST NOT require users to bridge internal representations when the framework can safely handle the conversion.
- SHOULD make routine orchestration automatic when the framework owns the participating operations.
- MUST document ordering and dependencies between operations when those relationships affect correctness.

#### 6.2 Clear ownership

- Every resource MUST have an identifiable ownership and lifecycle contract.
- MUST distinguish borrowed references, owned resources, shared resources, and transferred ownership when relevant.
- MUST document who is responsible for cleanup.
- MUST define ownership across asynchronous boundaries.
- MUST avoid ambiguous situations in which multiple parties believe they are responsible for the same cleanup operation.

#### 6.3 One source of truth

- MUST avoid redundant declarations of the same fact unless each serves a distinct purpose.
- MUST establish clear precedence when multiple configuration sources exist.
- MUST NOT require users to synchronize duplicate configuration manually when the framework can derive one value from another.
- SHOULD keep configuration near the scope where its effects are intended.

#### 6.4 Defaults and options

- Common tasks SHOULD work with minimal configuration.
- Defaults MUST be documented and predictable.
- Options MUST correspond to meaningful user decisions.
- MUST NOT expose internal implementation switches as public configuration without a clear user-facing purpose.
- SHOULD use named configuration properties for related optional settings.
- MUST define the behavior of omitted options and explicit overrides.
- Configuration resolution MUST be deterministic.

#### 6.5 Escape hatches

- Advanced users SHOULD have access to lower-level capabilities when necessary.
- Escape hatches MUST have clearly documented stability and support guarantees.
- MUST NOT require users to abandon the entire high-level model to customize one aspect.
- Lower-level access MUST NOT be mistaken for a guarantee that internal APIs are stable.

---

## Part V — Lifecycle and Asynchronous Behavior

### 7. Resource lifecycle

- Every acquired resource MUST have a defined cleanup strategy.
- Lifecycle operations SHOULD have consistent, semantically appropriate names.
- MUST document whether repeated initialization or cleanup is safe.
- MUST define behavior after initialization failure.
- MUST prevent partial failure from silently leaving resources in an invalid state.
- MUST document whether cleanup is synchronous or asynchronous.
- SHOULD support structured resource management where the language permits it.

Examples include subscriptions, event listeners, connections, mounted applications, file handles, and background tasks.

### 8. Asynchronous and concurrent operations

- MUST distinguish initiating an operation from completing it.
- MUST document the guarantees of returned promises, futures, handles, and callbacks.
- SHOULD support cancellation for long-running operations when meaningful and feasible.
- MUST define relevant cancellation and cleanup behavior.
- MUST document whether overlapping calls are serialized, rejected, merged, or allowed to run concurrently.
- MUST specify ordering guarantees when callers depend on them.
- MUST define how failures propagate across asynchronous boundaries.
- MUST clarify who owns work that continues after the initiating call returns.
- SHOULD prevent stale operations from unexpectedly overwriting newer state when the API promises latest-state behavior.

APIs MUST NOT imply stronger cancellation, ordering, or completion guarantees than the implementation provides.

---

## Part VI — Errors and Diagnostics

### 9. Error contracts

#### 9.1 Structured errors

Public errors MUST provide a stable machine-readable identity and sufficient information to diagnose the failure.

A common error representation SHOULD support: `code` (stable error identifier), `message` (human-readable description), `cause` or `details` (underlying cause or relevant contextual information), `hint` or `action` (recovery guidance when meaningful). These are semantic requirements, not a requirement that every exception expose four identically named fields.

#### 9.2 Error behavior

- MUST distinguish expected operational failures from programmer errors and internal defects where the language and domain permit.
- MUST use a consistent error-reporting strategy within related APIs.
- MUST preserve useful underlying causes when wrapping errors.
- MUST document errors callers are expected to handle.
- MUST NOT silently convert failures into success-shaped results.
- SHOULD fail immediately when an invalid state makes further processing unsafe or meaningless.
- MAY aggregate independent validation errors when that produces more useful feedback.
- MUST NOT retry or fall back silently when doing so could violate correctness or conceal an important failure.

#### 9.3 Actionable diagnostics

- Errors SHOULD identify the failed operation and relevant input or source location.
- MUST provide enough context to distinguish common failure cases.
- SHOULD explain a corrective action when one can be identified reliably.
- MUST NOT expose secrets or sensitive payloads in messages, logs, or diagnostic metadata.
- MUST avoid duplicating the same error at every layer without adding useful context.
- SHOULD preserve enough diagnostic information for debugging without requiring users to inspect internal implementation details.

#### 9.4 Boundary validation

- MUST validate public inputs before consequential processing when invalid values could cause unsafe work or confusing failures.
- MUST validate types, ranges, nullability, and invariants relevant to the operation.
- MUST validate untrusted input at the appropriate trust boundary.
- MUST distinguish input validation from authorization and other security checks.
- MUST NOT rely solely on compile-time types to validate data received at runtime.

---

## Part VII — Security and Performance

### 10. Security and trust boundaries

- MUST define trust boundaries where they materially affect the API.
- MUST validate untrusted inputs and enforce relevant authorization requirements.
- MUST make dangerous or irreversible operations sufficiently explicit.
- MUST protect secrets and sensitive data in errors, logs, and diagnostic output.
- SHOULD choose secure defaults.
- MUST document security-relevant limitations and required caller responsibilities.
- MUST NOT imply that data is trusted merely because it conforms to a type or schema.

### 11. Performance and resource costs

- SHOULD make the cost of common operations reasonable and predictable.
- MUST document material I/O, blocking, allocation, subscription, or rendering behavior when users need that knowledge to use the API correctly.
- MUST NOT conceal unexpectedly expensive work behind operations that conventionally imply trivial access.
- SHOULD avoid unnecessary repeated work caused by composition.
- MAY use caching, laziness, batching, and memoization when their semantics are clear.
- MUST preserve correctness when introducing performance optimizations.
- SHOULD provide advanced optimization controls only when they address meaningful user needs.
- MUST NOT force ordinary users to understand implementation-level performance machinery merely to accomplish common tasks.

---

## Part VIII — Documentation and Discoverability

### 12. Discoverability

- Public capabilities MUST be discoverable through the API's public organization and documentation.
- APIs SHOULD be grouped around tasks, domain concepts, or clear responsibilities.
- Related operations SHOULD have predictable locations.
- MUST avoid forcing users to understand internal package boundaries to locate public functionality.
- SHOULD use familiar domain terms in public entry points.
- MUST clearly distinguish stable, experimental, deprecated, and internal APIs.

### 13. Documentation

#### 13.1 Progressive documentation

Documentation SHOULD follow a progressive learning path:

1. Learn: Understand the problem, core concepts, and simplest successful example.
2. Use: Find exact APIs, options, contracts, and practical recipes.
3. Understand: Explore advanced behavior, architecture, rationale, and implementation details.

#### 13.2 Working examples

- Every major public capability SHOULD have a minimal working example.
- Examples MUST accurately represent supported public usage.
- Examples SHOULD demonstrate common tasks before advanced customization.
- MUST document important preconditions, limitations, and failure behavior.
- SHOULD show error handling when failure is a realistic part of the task.
- MUST keep examples synchronized with the supported API version.
- SHOULD automate example compilation or execution when feasible.

#### 13.3 Documentation quality

- Public contracts MUST be documented where users need them to make correct decisions.
- MUST document meaningful side effects, defaults, ownership, and compatibility guarantees.
- SHOULD explain why an option exists when its purpose is not obvious.
- MUST avoid requiring readers to infer essential behavior from implementation details.
- Internal architecture MAY be documented separately without making it a prerequisite for ordinary use.

---

## Part IX — Extensibility and Evolution

### 14. Extensibility

#### 14.1 Closed and open variation

- MUST use closed enumerations or equivalent constructs when the variation set is finite and controlled by the API owner.
- SHOULD use interfaces, traits, callbacks, or other extension mechanisms when users need to supply implementations.
- MUST distinguish user-extensible contracts from framework-owned behavior.
- SHOULD provide extension points at stable, meaningful boundaries.
- MUST NOT expose extension points that force users to depend on unstable internals.

#### 14.2 Customization boundaries

- Customization MUST have documented constraints and guarantees.
- SHOULD allow users to replace or customize one capability without reimplementing unrelated functionality.
- MUST define what the framework retains control over and what the user controls.
- SHOULD keep default behavior useful without customization.
- MUST NOT imply arbitrary extensibility when the design supports only a fixed set of implementations.

### 15. Backward compatibility and versioning

#### 15.1 Compatibility contract

Compatibility MUST be evaluated across: public method and type signatures; runtime behavior and observable side effects; defaults and configuration precedence; error codes and documented failure behavior; extension points and downstream implementations; ordering, lifecycle, and concurrency guarantees; generated artifacts and supported integration boundaries, where applicable.

#### 15.2 Additive changes

Minor and patch releases SHOULD preserve established contracts.

Potentially safe additions include new methods, optional configuration properties, and new capabilities. However, additions MUST be reviewed for conflicts with existing implementations, overloads, type inference, and behavioral assumptions.

The following changes MUST NOT be treated as automatically non-breaking: adding required members to public interfaces implemented by users; changing defaults in ways that alter behavior; introducing new overloads that change call resolution; narrowing accepted inputs; changing error or lifecycle semantics; adding behavior that violates previously documented guarantees.

Breaking changes SHOULD be reserved for major versions or another explicitly defined breaking-change mechanism.

#### 15.3 Deprecation

Deprecation SHOULD follow three stages:

1. Announce: Mark the old API as deprecated and identify its replacement.
2. Migrate: Provide migration instructions and retain the old API throughout the declared support period.
3. Remove: Remove the API only at a permitted breaking-change boundary, according to the published policy.

Deprecations MUST communicate: why the API is deprecated; what should replace it; how to migrate; when removal is expected; any behavioral differences in the replacement.

The migration period SHOULD be at least one major-version cycle when feasible, unless the published policy or a significant safety issue justifies a different approach.

#### 15.4 Stability tiers

Public APIs SHOULD have explicit stability levels, such as stable, experimental, deprecated, and internal or unsupported. Each level MUST have a clear meaning for compatibility and support. Internal APIs MUST NOT accidentally become implied public contracts through examples or documentation.

---

## Part X — Platform Consistency and Interoperability

### 16. Platform behavior

For APIs supporting multiple platforms or execution environments:

- MUST preserve shared public semantics across supported targets wherever feasible.
- MUST document meaningful target-specific differences.
- MUST define what happens when a capability is unsupported.
- MUST NOT claim portability beyond the guarantees actually provided.
- SHOULD isolate platform-specific capabilities behind explicit interfaces or clearly scoped extensions.
- MUST distinguish a shared API shape from a guarantee of identical runtime behavior.

### 17. Interoperability

- SHOULD follow established abstractions and conventions of the host language and ecosystem.
- MUST define how the API interacts with standard types, error mechanisms, asynchronous primitives, and resource-management patterns.
- SHOULD minimize unnecessary wrappers and conversions.
- MUST document integration boundaries and unsupported combinations.
- MUST avoid requiring users to abandon familiar language capabilities without a clear benefit.
- SHOULD interoperate with established tooling for type checking, debugging, diagnostics, and documentation.

---

## Part XI — Validation and Governance

### 18. Validate APIs through real usage

An API MUST NOT be considered well-designed solely because its implementation is internally consistent or its signatures pass review.

Before stabilizing a significant public API, SHOULD validate it through representative consumer tasks.

#### 18.1 Representative tasks

Tests SHOULD cover: a common task using defaults; a task requiring ordinary customization; a task involving composition with another capability; a realistic failure and its diagnosis; a lifecycle or asynchronous boundary, where applicable; a migration or compatibility scenario for established APIs.

#### 18.2 Evaluation criteria

Evaluate: whether users complete tasks correctly; whether users can discover the right entry point; how many concepts must be explained before work begins; whether users misunderstand defaults or observable behavior; whether unnecessary configuration or coordination is required; whether failures provide enough information for recovery; whether consumers need unsupported APIs, workarounds, or unsafe casts.

Where appropriate, compare alternative designs using representative users and tasks rather than relying exclusively on author preference.

#### 18.3 Evidence and iteration

- MUST record significant design decisions and their rationale.
- SHOULD use observed confusion, errors, and workarounds to identify API problems.
- SHOULD revise the conceptual model before adding more exceptions to an awkward API.
- MUST NOT preserve an established design merely because it has already been implemented.
- SHOULD validate the smallest representative use case before expanding the API's scope.

### 19. API review checklist

Before approving a significant public API, reviewers SHOULD answer the following questions.

- **Understandability.** Can users explain the public concepts in ordinary domain language? Is the simplest correct path obvious? Are names consistent and semantically accurate? Does the design introduce unnecessary concepts?
- **Correctness.** Are inputs, outputs, and errors predictable? Are important invariants expressed in types where practical? Are mutation and side effects explicit? Are lifecycle and concurrency guarantees clear?
- **Composition.** Do related operations fit together naturally? Is ownership unambiguous? Does the framework perform routine orchestration where appropriate? Are configuration sources and precedence clear?
- **Safety.** Are trust boundaries and dangerous operations handled appropriately? Are failures actionable? Are sensitive details protected? Are resource costs and cleanup responsibilities understood?
- **Discoverability.** Can users find the correct API? Are working examples available? Can users learn basic usage without studying internals? Are advanced capabilities documented without overwhelming beginners?
- **Evolution.** Are stability guarantees clear? Have downstream implementations and inferred types been considered? Are deprecation and migration policies defined? Can the API evolve without unnecessary breakage?
- **Validation.** Has the design been exercised through realistic consumer code? Have unnecessary decisions and coordination been identified? Do observed results support the chosen abstraction?

A checklist is a review aid, not a substitute for judgment. Reviewers MUST evaluate the API as a coherent whole.

---

## Part XII — Applying the Standard

### 20. Recommended design workflow

Apply the principles in this order:

1. Identify the user task. Establish what the consumer is trying to accomplish.
2. Define the public concepts. Decide what users need to understand, independently of implementation details.
3. Design the simplest successful path. Specify the minimum API and defaults needed for a useful result.
4. Define contracts. Establish input, output, error, state, lifecycle, and side-effect semantics.
5. Design composition and customization. Determine how capabilities connect and where users need control.
6. Document and demonstrate. Create a working example and explain the public model.
7. Validate through a consumer. Implement a representative task and identify confusion, unnecessary decisions, and workarounds.
8. Refine the model. Change abstractions when the evidence points to a conceptual problem instead of accumulating special cases.
9. Establish evolution guarantees. Specify stability, versioning, deprecation, and migration policies.
10. Review the complete contract. Ensure the API's concepts, signatures, behavior, and documentation agree.

For large frameworks, validate a small representative application before designing every feature. This reveals whether the public model works in practice without committing to a speculative architecture.

---

## References and supporting guidance

The principles above synthesize general API design reasoning with established published guidance. The following sources support the relevant areas but do not experimentally validate every individual rule.

1. Bogner, Kotstein, and Pfaff (2023), empirical study of REST API design rules and understandability. DOI: [10.1007/s10664-023-10367-y](https://doi.org/10.1007/s10664-023-10367-y)
2. Rust API Guidelines, checklist covering naming, documentation, predictability, flexibility, type safety, dependability, debugging, and future-proofing. [Official checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
3. Google Cloud, API Design Guide, covering resource design, methods, errors, documentation, versioning, and backward compatibility. [Official guide](https://docs.cloud.google.com/apis/design)
4. Microsoft, Azure service design considerations, covering developer experience, abstraction, terminology, consuming applications, and feedback. [Official guidance](https://github.com/microsoft/api-guidelines/blob/vNext/azure/ConsiderationsForServiceDesign.md)

**Final standard.** A well-designed API is not simply one with consistent names, short signatures, and good error types. It is an API whose concepts are understandable, whose behavior is predictable, whose capabilities compose naturally, whose defaults minimize unnecessary decisions, and whose contracts support safe evolution. The ultimate test is whether users can accomplish real tasks correctly without having to understand or coordinate complexity that the API could reasonably manage for them.
