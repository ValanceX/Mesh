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
As of 2026-09-22: Pass 1 (elements, attributes, string values, plain text),
Pass 2 (the `{...}` expression syntax in both attribute values and child
content — covering `Literal` (String/Number/Boolean/Null), `Reference`,
and `MemberAccess`; the `tag_name`/`identifier` lexical split; and string
escape decoding), and **Pass 3a** (`UnaryExpression`/`BinaryExpression`/
`ConditionalExpression` — §5's precedence ladder — plus the unsigned
`number` token fix above that resolves the signed-literal/unary-minus
ambiguity) are shipped, per
`docs/superpowers/specs/2026-09-22-mesh-v0.1-pass-3-5-outline.md`'s 3a/3b
split; Pass 3a's `mesh-syntax`/`mesh-semantic` type shapes are recorded
in §8 below. **Pass 3b** (`ArrayExpression`/`ObjectExpression`/
`CommandInvocation`/`EventValue`) is being planned now, informed by real
Pass 3a implementation experience as intended — its `mesh-syntax`/
`mesh-semantic` type shapes are now fixed in §8 below, ahead of its
`writing-plans` cycle, same treatment Pass 3a's shapes got. **Not yet
implemented:** Pass 3b (as above), §4's event bindings (Pass 4), and
§3's nested elements (Pass 4). The rest of this document is the target
those remaining passes implement against.

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
number      ::= digit+ ( '.' digit+ )?
digit       ::= [0-9]
whitespace  ::= [ \t\n\r]+   (* not significant; separates tokens *)
```

**`tag_name` vs. `identifier` — these are deliberately two different
tokens, not one.** `tag_name` allows hyphens (`user-card`, matching
HTML/JSX custom-element convention) and is used *only* for element names.
`identifier` forbids hyphens and is used everywhere else: attribute names,
references, member-access properties, event names, command names. This
matters once Pass 3a introduces `-` as binary subtraction — if references
allowed hyphens, `a-b` would be lexically ambiguous between "one
identifier" and "`a` minus `b`". JSX has the same split for the same
reason. **This split is implemented as of Pass 2**: the `attribute` rule
now uses the hyphen-free `identifier` token for attribute names (distinct
from `tag_name`, which still allows hyphens for element names), avoiding
the ambiguity before Pass 3a introduces binary operators.

**`number` no longer allows a leading `-` (revised for Pass 3a).** Pass 2
shipped `number ::= '-'? digit+ (...)`, matching a signed literal like
`-3.5` as one token. Pass 3a's §5 grammar adds unary `-` as a real
operator, which collides with a signed `number` token: input `-3` would
be simultaneously valid as a single `number_literal` token *and* as
`unary_expression`'s `'-' unary_expression` production applied to `3` —
two different parse trees for the same two characters, an outright
ambiguity, not just an edge case. The fix is the standard C-family
resolution: `number` itself is always unsigned, and a negative numeric
literal is represented as `UnaryExpression{ operator: Negate, operand:
Literal::Number(...) }` like any other negated expression (`-x`,
`-(a+b)`) — one representation for "negative", not two. This changes
previously-shipped Pass 2 behavior: `{-3.5}` now lowers to a `Unary`
node wrapping `Literal::Number("3.5")`, not a `Literal::Number("-3.5")`
directly. `NumberLiteral.value` (the raw source text) correspondingly
never contains a leading `-` after this change.

**`event_value` is a single atomic token** (clarified for Pass 3b
planning), not `'$'` followed by a separately-tokenized `identifier`.
Written as `'$' identifier` above for readability, but implemented as
one regex token with no whitespace permitted between `$` and the name —
`$event` is valid, `$ event` (with a space) is not. This follows directly
from `event_value` living in this section (§2, *lexical* grammar) rather
than §5 (*structural* grammar): a structural `seq('$', identifier)` rule
would let Tree-sitter's automatic whitespace-insertion between tokens
silently accept the spaced form, which doesn't fit "one lexical token."
`command_invocation` (`identifier '(' ...`, §5) stays a structural rule
by contrast — whitespace before its `(` is unremarkable, same as
anywhere else in the structural grammar.

**Comments:** not supported in v0.1. MPRX is primarily compiled/generated
rather than hand-authored, and nothing in the existing architecture calls
for them. Revisit if real usage shows a need.

**String escapes:** `\"`, `\\`, `\n`, `\t`. Escape decoding is implemented
as of Pass 2 — a literal `"` inside a string is representable via `\"`,
and both `mesh-parser`'s attribute-value strings and string literals used
inside `{...}` expressions decode escapes through the same `lower_string`
path.

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
are valid in child/content position as of Pass 2, not just in
attribute values — `docs/ARCHITECTURE.md`'s own example,
`<text>{user.name}</text>`, puts an expression directly in element
content, and `container_element`'s grammar now uses `repeat($.child)` so
an element can hold any number of text/expression children in any order.

**Whitespace in child content (open question for Pass 4):** a
whitespace-only text run between children (e.g. the newline/indentation in
`<title>\n  {user}\n</title>`) currently produces a literal `Child::Text`
node containing that whitespace, exactly matching `text ::= [^<{]+` — MPRX
does not trim or collapse whitespace-only text children the way JSX trims
whitespace-only `JSXText`. This is the current, shipped, tested behavior
for Pass 2 (see `crates/mesh-parser/tests/parse.rs`), not a final
decision: whether to keep it as-is or add JSX-style trimming is an open
question to revisit deliberately at Pass 4, once nested elements make
multi-child content the common case.

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

| Node kind                                   | Introduced in                        | `mesh-syntax` type                                       | `mesh-semantic` type            |
| ------------------------------------------- | ------------------------------------ | -------------------------------------------------------- | ------------------------------- |
| Element                                     | Pass 1                               | `Element`                                                | `Element`                       |
| Attribute                                   | Pass 1                               | `Attribute`                                              | `Attribute`                     |
| StringLiteral (attr value)                  | Pass 1                               | `StringLiteral`                                          | `String` (inline)               |
| Text (child)                                | Pass 1                               | `Text`                                                   | `Text`                          |
| AttributeValue                              | Pass 1 (revised Pass 2)              | `AttributeValue`                                         | `AttributeValue`                |
| Expression (wrapper)                        | Pass 2                               | `Expression`                                             | `Expression`                    |
| Literal (String/Number/Boolean/Null)        | Pass 2                               | `Literal`                                                | `Literal`                       |
| Reference                                   | Pass 2                               | `Reference`                                              | `Expression::Reference(String)` |
| MemberAccess                                | Pass 2                               | `MemberAccess`                                           | `Expression::MemberAccess{..}`  |
| Child (Text \| Expression)                  | Pass 2                               | *extends existing `Vec<Text>` to a child enum — see §10* | same                            |
| UnaryExpression                             | Pass 3a                              | `UnaryExpression` (+ `UnaryOperator`)                     | `Expression::Unary{operator,operand}` (reuses `mesh_syntax::UnaryOperator`) |
| BinaryExpression                            | Pass 3a                              | `BinaryExpression` (+ `BinaryOperator`)                   | `Expression::Binary{operator,left,right}` (reuses `mesh_syntax::BinaryOperator`) |
| ConditionalExpression                       | Pass 3a                              | `ConditionalExpression`                                  | `Expression::Conditional{condition,consequent,alternate}` |
| ArrayExpression                             | Pass 3b                              | `ArrayExpression`                                        | `Expression::Array(Vec<Expression>)` |
| ObjectExpression                            | Pass 3b                              | `ObjectExpression` (+ `ObjectMember`, `ObjectKey`)        | `Expression::Object(Vec<ObjectMember>)` (IR-local `ObjectMember{key: String, value}` — `ObjectKey` flattens, see note below) |
| CommandInvocation                           | Pass 3b                              | `CommandInvocation`                                      | `Expression::Command{command,arguments}` |
| EventValue                                  | Pass 3b (alongside CommandInvocation) | `EventValue`                                             | `Expression::EventValue(String)` |
| EventBinding                                | Pass 4                               | `EventBinding`                                           | TBD at Pass 4 planning          |
| Child::Element (nested)                     | Pass 4                               | extends `Child` enum                                     | same                            |
| Diagnostics (plural, from both parse+lower) | Pass 4                               | n/a — changes `parse`/`lower` signatures                 | n/a                             |

Pass 4's exact Rust type shapes are intentionally left "TBD at
planning" — this spec fixes the *grammar and semantics*, not Rust API
signatures, which is exactly the layer that should still get decided at
`writing-plans` time per real implementation experience (same reasoning
the Pass 3-5 outline already gave for not writing bite-sized plans this
far ahead). Pass 3a's and Pass 3b's shapes are fixed above as an
exception in each case: both brainstorms happened immediately ahead of
their own `writing-plans` cycle, so there was no gap between deciding
and implementing to leave open.

**`mesh-semantic` operator-enum reuse (Pass 3a):** `UnaryOperator` and
`BinaryOperator` are the first `mesh-syntax` types the Semantic IR
reuses directly instead of mirroring span-free. Every other AST type
mirrored so far (`Literal`, `Expression`, etc.) carries a `Span` that the
IR must strip — these operator enums never had one, so there's nothing
to strip and no reason to redeclare an identical enum in `mesh-semantic`.
Pass 3b's `ObjectKey` does *not* get this treatment: its two variants
(`Identifier(String)` vs `String(StringLiteral)`) carry a span on the
`StringLiteral` side and are semantically equivalent once lowered (both
just name a field), so the IR flattens `ObjectMember`'s key to a plain
`String` rather than reusing or mirroring `ObjectKey` — a genuine lowering
step, not a reuse.

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
  was amended per this spec before execution and has now shipped: (a)
  `identifier` was split into `tag_name` (element names) vs. hyphen-free
  `identifier` (attribute names, references, member-access properties);
  (b) expression support was extended to child/content position, not just
  attribute values; (c) string escape handling was added; (d) Boolean/Null
  literals were added alongside String/Number.
- **`docs/superpowers/specs/2026-09-22-mesh-v0.1-pass-3-5-outline.md`**'s
  previously-open questions (exact operator set, precedence, event-binding
  token shape, array/object syntax) are now resolved by §4-§5 above. The
  outline's cross-cutting risk assessment for Pass 4 (children model
  change, diagnostic-plurality change) still stands — this spec doesn't
  resolve Rust-level signatures, only grammar/semantics. The outline's own
  Pass 3 section has since been updated (same date) to reflect the 3a/3b
  split and 3a's fixed Rust type shapes — see that document directly
  rather than re-deriving it here.
- **This revision (2026-09-22, second pass)** fixes §8's Pass 3a row
  shapes (`UnaryExpression`/`BinaryExpression`/`ConditionalExpression`
  and their `mesh-semantic` mirrors) ahead of Pass 3a's `writing-plans`
  cycle, and splits the former single "Pass 3" into Pass 3a (this) and
  Pass 3b (`ArrayExpression`/`ObjectExpression`/`CommandInvocation`/
  `EventValue`, deferred until 3a ships and can be reviewed).
- **This revision (2026-09-22, third pass)**, written after Pass 3a
  shipped: fixes §8's Pass 3b row shapes (`ArrayExpression`,
  `ObjectExpression`/`ObjectMember`/`ObjectKey`, `CommandInvocation`,
  `EventValue`) ahead of Pass 3b's `writing-plans` cycle, and adds §2's
  `event_value`-is-an-atomic-token clarification — a real lexical-grammar
  decision surfaced by actually designing the grammar rule, the same way
  Pass 3a's number/unary-minus ambiguity was surfaced by designing that
  pass's grammar rather than being visible from the EBNF alone.
