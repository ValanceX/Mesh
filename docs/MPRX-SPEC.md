# MPRX Language Specification

This is the canonical syntax reference for MPRX (MeshExpr), the declarative
UI language owned by MESH. It is a **living document**: updated as each
v0.1 pass ships, not a one-time design proposal. Every grammar/AST
implementation task should be written against this spec rather than
inventing syntax inline — see `docs/superpowers/plans/` for the
implementation plans that build it out incrementally.

For the conceptual "why" behind MPRX (what it is, what it deliberately
excludes, how it relates to NEXUS/PORT), see `docs/ARCHITECTURE.md`. This
document covers the "what" — precise grammar and per-construct semantics.

**Status:** describes the full v0.1 grammar target. The "Introduced in"
column throughout marks what's actually implemented today vs. planned.
As of 2026-09-22: Pass 1 (elements, attributes, string values, plain text)
is shipped. Nothing past that is implemented yet — the rest of this
document is the target Pass 2-5 implements against.

---

## 1. Design principles (recap)

- MPRX is a declarative UI language, not a general-purpose one. See
  `docs/ARCHITECTURE.md` §4 for the full philosophy.
- Expressions are side-effect free. MPRX represents intent
  (`CommandInvocation`); it never executes application logic itself.
- The grammar is deliberately small. Anything not in this document is out
  of scope until a concrete need promotes it in (see §9).

---

## 2. Lexical grammar

```ebnf
tag_name    ::= [a-zA-Z_][a-zA-Z0-9_-]*
identifier  ::= [a-zA-Z_][a-zA-Z0-9_]*
event_value ::= '$' identifier
string      ::= '"' string_char* '"'
string_char ::= [^"\\] | '\\' escape
escape      ::= '"' | '\\' | 'n' | 't'
number      ::= '-'? digit+ ( '.' digit+ )?
digit       ::= [0-9]
whitespace  ::= [ \t\n\r]+   (* not significant; separates tokens *)
```

**`tag_name` vs. `identifier` — these are deliberately two different
tokens, not one.** `tag_name` allows hyphens (`user-card`, matching
HTML/JSX custom-element convention) and is used *only* for element names.
`identifier` forbids hyphens and is used everywhere else: attribute names,
references, member-access properties, event names, command names. This
matters once Pass 3 introduces `-` as binary subtraction — if references
allowed hyphens, `a-b` would be lexically ambiguous between "one
identifier" and "`a` minus `b`". JSX has the same split for the same
reason. **This retroactively affects the shipped Pass 1 grammar**, whose
`attribute` rule currently reuses the hyphen-allowing token for attribute
names — not a live bug yet (Pass 1 has no binary operators to collide
with), but it must be corrected before Pass 3, and naturally belongs in
Pass 2's grammar work since Pass 2 is the first pass to introduce
`reference`.

**Comments:** not supported in v0.1. MPRX is primarily compiled/generated
rather than hand-authored, and nothing in the existing architecture calls
for them. Revisit if real usage shows a need.

**String escapes:** `\"`, `\\`, `\n`, `\t`. The shipped Pass 1 grammar has
no escape handling at all (a literal `"` inside a string is currently
unrepresentable) — this is a gap this spec closes, to be picked up
whenever grammar work next touches the `string` rule (Pass 2).

---

## 3. Structural grammar

```ebnf
document              ::= element
element                ::= self_closing_element | container_element
self_closing_element   ::= '<' tag_name element_modifier* '/>'
container_element      ::= '<' tag_name element_modifier* '>' child* '</' tag_name '>'
element_modifier        ::= attribute | event_binding
attribute               ::= identifier '=' attribute_value
attribute_value          ::= string | expression_block
child                    ::= text | expression_block | element
text                     ::= [^<{]+
expression_block          ::= '{' expression '}'
```

`child` allows three things: plain text, an `{expr}` block, or a nested
`element`. **Nested elements are Pass 4 scope** (see §8's pass column) —
until then, `child` is effectively `text | expression_block`. Expressions
are valid in child/content position from Pass 2 onward, not just in
attribute values — `docs/ARCHITECTURE.md`'s own example,
`<text>{user.name}</text>`, puts an expression directly in element
content. The already-written Pass 2 plan only threaded `{...}` through
attribute values; it needs amending to cover child content too (tracked in
§10).

Open/close tag name matching (`<foo>...</bar>` should be rejected) is
**Pass 4** structural-validation scope, same as the existing roadmap spec
already states — not re-litigated here.

---

## 4. Event bindings

```ebnf
event_binding ::= 'on' '.' identifier '=' expression_block
```

Example: `on.select={selectUser($event)}`. Distinct from `member_access`
even though both use `.` — an event binding is a declaration (this element
emits this event, handle it with this expression), not a value expression,
so it gets its own grammar rule rather than reusing `member_access`.
**Pass 4 scope.**

---

## 5. Expression grammar

```ebnf
expression        ::= conditional_expression
conditional_expression ::= logical_or_expression ( '?' expression ':' expression )?
logical_or_expression  ::= logical_and_expression ( '||' logical_and_expression )*
logical_and_expression ::= equality_expression ( '&&' equality_expression )*
equality_expression     ::= relational_expression ( ( '==' | '!=' ) relational_expression )*
relational_expression   ::= additive_expression ( ( '<' | '<=' | '>' | '>=' ) additive_expression )*
additive_expression      ::= multiplicative_expression ( ( '+' | '-' ) multiplicative_expression )*
multiplicative_expression ::= unary_expression ( ( '*' | '/' | '%' ) unary_expression )*
unary_expression           ::= ( '!' | '-' ) unary_expression | postfix_expression
postfix_expression          ::= member_access | primary_expression
member_access                ::= ( reference | member_access ) '.' identifier
primary_expression            ::= literal | reference | event_value | array_expression
                                 | object_expression | command_invocation | '(' expression ')'
reference                      ::= identifier
literal                         ::= string | number | 'true' | 'false' | 'null'
array_expression                 ::= '[' ( expression ( ',' expression )* ','? )? ']'
object_expression                 ::= '{' ( object_member ( ',' object_member )* ','? )? '}'
object_member                      ::= ( identifier | string ) ':' expression
command_invocation                  ::= identifier '(' ( expression ( ',' expression )* )? ')'
```

**Precedence, highest to lowest:** postfix/member-access → unary (`!`,
`-`) → multiplicative (`*` `/` `%`) → additive (`+` `-`) → relational (`<`
`<=` `>` `>=`) → equality (`==` `!=`) → logical AND (`&&`) → logical OR
(`||`) → conditional (`? :`, right-associative). This is the standard
C-family precedence ladder — no MPRX-specific reordering.

**Literals:** String and Number were Pass 2's originally-stated scope;
Boolean (`true`/`false`) and Null (`null`) are added here since they're
the same `Literal` category (not a new node kind) and near-certain to be
needed as soon as anyone writes `disabled={true}` — cheap to include now
rather than as a surprise gap later.

**`event_value`** (`$event`) is its own node kind, not a `reference` whose
name happens to start with `$` — `docs/ARCHITECTURE.md`'s own
`CommandInvocation` example names "EventValue" as a distinct thing
(`├── command: selectUser └── arguments └── $event`). The lexical rule is
general (`'$' identifier`) so future special values (if any) don't need a
grammar change, but v0.1 semantics only meaningfully understand `$event` —
the compiler doesn't validate the set of legal `$`-names (structural
validation only, consistent with the rest of v0.1).

**Object literals need doubled braces when used as an attribute/child
value** — `data={{ key: "value" }}` — because the outer `{}` is the
`expression_block` wrapper (the "this is an expression, not text" marker)
and the inner `{}` is the object literal itself. This is the same
resolution JSX uses for the identical ambiguity, not an invented MPRX
quirk.

**Explicitly not in the expression grammar:** array/property subscript
access (`a[0]`, `a["key"]`) — `docs/ARCHITECTURE.md` never mentions it,
and `ArrayExpression` there is a literal constructor, not an accessor. Add
it later only against a concrete need.

---

## 6. Command invocations and NEXUS

`CommandInvocation` (`selectUser($event)`) represents *intent*, per
`docs/ARCHITECTURE.md` §6 — MESH only records that this command was
invoked with these arguments; NEXUS resolves what it actually does. The
grammar allows `command_invocation` anywhere an `expression` is valid for
consistency, but its documented, intended use is as the body of an
`event_binding`'s `expression_block` — `on.select={selectUser($event)}` —
not general-purpose composition like `1 + selectUser($event)`.

---

## 7. Optional chaining — explicitly deferred

`docs/ARCHITECTURE.md` §5 notes optional chaining (`user?.profile?.name`)
"may eventually be supported." It is **not** in this grammar. Do not add
`?.` until there's a concrete driving need — this spec exists partly to
prevent exactly this kind of speculative addition.

---

## 8. Node kind → AST/IR mapping

Every node kind, which pass introduces it, and its `mesh-syntax` /
`mesh-semantic` type names. Implementation plans should reference this
table rather than re-deriving names per pass.

| Node kind | Introduced in | `mesh-syntax` type | `mesh-semantic` type |
|---|---|---|---|
| Element | Pass 1 | `Element` | `Element` |
| Attribute | Pass 1 | `Attribute` | `Attribute` |
| StringLiteral (attr value) | Pass 1 | `StringLiteral` | `String` (inline) |
| Text (child) | Pass 1 | `Text` | `Text` |
| AttributeValue | Pass 1 (revised Pass 2) | `AttributeValue` | `AttributeValue` |
| Expression (wrapper) | Pass 2 | `Expression` | `Expression` |
| Literal (String/Number/Boolean/Null) | Pass 2 | `Literal` | `Literal` |
| Reference | Pass 2 | `Reference` | `Expression::Reference(String)` |
| MemberAccess | Pass 2 | `MemberAccess` | `Expression::MemberAccess{..}` |
| Child (Text \| Expression) | Pass 2 | *extends existing `Vec<Text>` to a child enum — see §10* | same |
| UnaryExpression | Pass 3 | `UnaryExpression` | TBD at Pass 3 planning |
| BinaryExpression | Pass 3 | `BinaryExpression` | TBD at Pass 3 planning |
| ConditionalExpression | Pass 3 | `ConditionalExpression` | TBD at Pass 3 planning |
| ArrayExpression | Pass 3 | `ArrayExpression` | TBD at Pass 3 planning |
| ObjectExpression | Pass 3 | `ObjectExpression` | TBD at Pass 3 planning |
| CommandInvocation | Pass 3 | `CommandInvocation` | TBD at Pass 3 planning |
| EventValue | Pass 3 (alongside CommandInvocation) | `EventValue` | TBD at Pass 3 planning |
| EventBinding | Pass 4 | `EventBinding` | TBD at Pass 4 planning |
| Child::Element (nested) | Pass 4 | extends `Child` enum | same |
| Diagnostics (plural, from both parse+lower) | Pass 4 | n/a — changes `parse`/`lower` signatures | n/a |

Pass 3/4's exact Rust type shapes are intentionally left "TBD at planning"
— this spec fixes the *grammar and semantics*, not Rust API signatures,
which is exactly the layer that should still get decided at
`writing-plans` time per real implementation experience (same reasoning
the Pass 3-5 outline already gave for not writing bite-sized plans this
far ahead).

---

## 9. Explicitly out of scope for v0.1

- Comments (§2)
- Array/object subscript access (§5)
- Optional chaining (§7)
- Slots, component composition beyond flat props (`docs/ARCHITECTURE.md`
  §10 calls these out as eventual, not v0.1)
- Any component-model type checking (resolving `Reference`/`MemberAccess`
  against a real prop schema) — structural parsing/representation only,
  per the v0.1 roadmap spec's non-goals

---

## 10. Impact on already-written plans

- **`docs/superpowers/plans/2026-09-22-mesh-v0.1-pass-2-expressions.md`**
  needs amending before execution: (a) split `identifier` into `tag_name`
  (element names) vs. hyphen-free `identifier` (attribute names,
  references, member-access properties); (b) extend expression support to
  child/content position, not just attribute values; (c) add string escape
  handling; (d) add Boolean/Null literals alongside String/Number. See the
  updated plan file — this spec's approval is what authorizes rewriting it.
- **`docs/superpowers/specs/2026-09-22-mesh-v0.1-pass-3-5-outline.md`**'s
  previously-open questions (exact operator set, precedence, event-binding
  token shape, array/object syntax) are now resolved by §4-§5 above. The
  outline's cross-cutting risk assessment for Pass 4 (children model
  change, diagnostic-plurality change) still stands — this spec doesn't
  resolve Rust-level signatures, only grammar/semantics.
