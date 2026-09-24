# MPRX Language Specification

MPRX (MeshExpr) is the declarative UI language at the heart of [MESH](../README.md). This document is the **canonical syntax reference**: the exact grammar and the rules for each construct.

- **New to MPRX?** Read the [tour](#mprx-in-five-minutes) below, then skim §3–§5.
- **Want the "why"?** See the [Architecture doc](./ARCHITECTURE.md).
- **Want a gentler introduction?** The [Writing MPRX guide](./guides/writing-mprx.md) covers the same ground with examples and common mistakes.
- **Implementing something?** Write against this spec rather than inventing syntax inline. The plans in `docs/superpowers/plans/` build it out step by step.

This is a **living document**. It was updated as each pass shipped and now describes MESH v0.2: §2–§8 are the syntax and its representation, and §9 is what a file means when it's checked against a component manifest. The "Introduced in" column in §8 shows which pass added each node kind.

---

## MPRX in five minutes

An MPRX document is a single element tree:

```xml
<page title="Users">
  <text>{user.name}</text>
  <avatar src={user.avatar} size={compact ? "sm" : "md"} />
  <button disabled={!user.active} on.click={selectUser($event)}>Select</button>
</page>
```

What's going on:

- **Elements** use HTML-style tags. Names may contain hyphens (`user-card`). Tags either close themselves (`<avatar ... />`) or wrap children (`<text>...</text>`).
- **Attributes** take either a plain string (`title="Users"`) or an expression in braces (`src={user.avatar}`).
- **Expressions** in `{...}` can read values (`user.name`), combine them (`!user.active`, `a + b`, `x ? "sm" : "md"`), and build arrays or objects (`{[1, 2]}`, `{{ key: "value" }}`). They can never *do* anything: every expression is side-effect free.
- **Event bindings** (`on.click={...}`) say what should happen when an element emits an event. The handler is usually a **command invocation** like `selectUser($event)`. MESH only records this intent, and NEXUS decides what it does.
- **`$event`** refers to the value the event carries.

What's intentionally *missing*: comments, subscripts (`a[0]`), optional chaining (`a?.b`), and arbitrary code. §10 lists everything that's out of scope.

Checked against a component manifest, every tag, attribute, event, name and command must be declared, and every value must have the right type (§9).

## Implementation status

| Pass | What it added | Status |
|---|---|---|
| 1 | Elements, attributes, string values, plain text | ✅ Shipped |
| 2 | `{...}` expressions in attributes and content: literals, references, member access | ✅ Shipped |
| 3a | Unary, binary, and conditional operators (§5's precedence ladder) | ✅ Shipped |
| 3b | Arrays, objects, command invocations, `$event` | ✅ Shipped |
| 4a | Multiple diagnostics per run (`ParseResult`/`LowerResult`), nested elements, close-tag capture | ✅ Shipped |
| 4b | Event bindings (§4); structural validation: duplicate attributes and bindings (warnings), mismatched close tags (errors) | ✅ Shipped |
| 5 | Canonical example and polished diagnostic rendering | ✅ Shipped |

v0.2 changes no syntax. Its passes (outlined in `docs/superpowers/specs/2026-09-24-mesh-v0.2-outline.md`) add checking:

| v0.2 pass | What it added | Status |
|---|---|---|
| 1 | Located syntax errors, and a stable code on every diagnostic | ✅ Shipped |
| 2 | Source spans in the Semantic IR (§8) | ✅ Shipped |
| 3 | The component manifest ([reference](./manual/manifest.md)) | ✅ Shipped |
| 4 | Resolving tags, props, events, references and commands against the manifest (§9.1, §9.5) | ✅ Shipped |
| 5a | Expression types (§9.2–§9.4) | ✅ Shipped |
| 5b | Checking values against their declared types (§9.6) | ✅ Shipped |
| 6 | JSON diagnostics output, and these semantics written down | ✅ Shipped |

*Last updated 2026-09-24. v0.2 is released as v0.2.0, and v0.1 as v0.1.0 (see the [v0.2](./releases/v0.2.md) and [v0.1](./releases/v0.1.md) release notes). v0.1's Pass 4 details are in `docs/superpowers/specs/2026-09-23-mesh-v0.1-pass-4-design.md`, and its Pass 5 is outlined in `docs/superpowers/specs/2026-09-22-mesh-v0.1-pass-3-5-outline.md`. Any grammar or semantics work beyond v0.2 starts a new spec.*

---

## 1. Design principles (recap)

- MPRX is a declarative UI language, not a general-purpose one. See
  `docs/ARCHITECTURE.md` §4 for the full philosophy.
- Expressions are side-effect free. MPRX represents intent
  (`CommandInvocation`); it never executes application logic itself.
- The grammar is deliberately small. Anything not in this document is out
  of scope until a concrete need promotes it in (see §10).

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

**Comments:** not supported. MPRX is primarily compiled/generated
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
`element`. Expressions are valid in child/content position as of Pass 2,
not just in attribute values — `docs/ARCHITECTURE.md`'s own example,
`<text>{user.name}</text>`, puts an expression directly in element
content, and `container_element`'s grammar now uses `repeat($.child)` so
an element can hold any number of text/expression children in any order.

**Nesting limit:** a file may nest at most 128 levels deep. Every
element counts as a level, and so does every expression inside another
one and every pair of parentheses, so `<a><b x={-(y)} /></a>` is 5
levels deep at `y`. A deeper file is rejected by `mesh-parser` with a
`nesting-too-deep` error, like a syntax error: it gets no AST, and so
no IR. The limit (`mesh_parser::MAX_NESTING_DEPTH`) keeps the
recursive stages after parsing within a small, fixed stack.

**Whitespace in child content:** whitespace-only text children are
formatting and are omitted from the semantic model; text containing any
non-whitespace character is preserved verbatim, with no trimming,
collapsing, or normalization. "Whitespace-only" is exactly Rust's
`str::trim().is_empty()` (Unicode-aware), applied during AST -> IR
lowering (`mesh-semantic::lower_child`) — the CST and AST both retain
every text node exactly as parsed, including whitespace-only ones, for
tooling/source-location/diagnostic purposes. Only the IR omits them. See
`crates/mesh-semantic/tests/lower.rs` for the shipped, tested behavior.

**Open/close tag-name matching** is implemented as of Pass 4b, in
`mesh-semantic::lower`, not in the grammar — `<foo>...</bar>` still
parses successfully (the grammar captures the close tag's `tag_name`
without a back-reference to the open tag), but lowering it reports an
`Error`-severity diagnostic and still produces usable IR: the mismatch is
non-fatal, lowering continues, and the resulting `Element.name` is always
the *opening* tag's name. Comparison is exact Rust string equality — no
case-folding or Unicode normalization. Self-closing elements are never
checked (there is no close tag to mismatch against). See
`crates/mesh-semantic/tests/lower.rs` for the shipped, tested behavior.

**Duplicate attributes and duplicate event bindings** are also a Pass 4b
`mesh-semantic::lower` policy: for each attribute name (respectively,
event-binding name) that appears more than once on an element, the
semantic IR keeps only that name's final source occurrence, in the
surviving entries' own winning-occurrence source order; every shadowed
occurrence is dropped from the IR and reported as a `Warning`-severity
diagnostic, ordered by the shadowed occurrence's own source position.
Attributes and event bindings are deduplicated independently of one
another — an attribute named `click` and an `on.click` event binding
never collide. See `crates/mesh-semantic/tests/lower.rs` for the shipped,
tested behavior.

---

## 4. Event bindings

```ebnf
event_binding ::= 'on' '.' identifier '=' expression_block
```

Example: `on.select={selectUser($event)}`. Distinct from `member_access`
even though both use `.` — an event binding is a declaration (this element
emits this event, handle it with this expression), not a value expression,
so it gets its own grammar rule rather than reusing `member_access`.

**Implemented as of Pass 4b.** `'on' '.'` is lexed as a single atomic
token (`token(seq('on', '.'))` in `grammar.js`), which is what lets a
plain attribute literally named `on` (`on="x"`) and a real event binding
(`on.click={...}`) coexist with no `prec`/`conflicts` declarations
needed — the two productions diverge at the token immediately following
the shared `on` prefix. `EventBinding.name` is extracted only from the
grammar's `field('name', $.identifier)` node and therefore never includes
the `on.` prefix token — `on.click={...}` always yields `name ==
"click"`, never `"on.click"`. Without a component manifest, any
identifier is accepted as an event name (there is no registry of known
DOM/runtime events), and the handler expression isn't checked. Against a
manifest, the event must be one the element's component declares, and
the handler must be a command invocation (§9.1, §9.5). `mesh-syntax`/
`mesh-semantic` type shapes are recorded in §8 below.

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
grammar change, but only `$event` means anything. Without a component
manifest, the compiler doesn't validate the set of legal `$`-names; against
one, any other `$` name is an error, and `$event` may appear only where
§9.5 allows.

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
not general-purpose composition like `1 + selectUser($event)`. Checked
against a component manifest, that is the only place a command may appear
(§9.5).

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
| StringLiteral (attr value)                  | Pass 1                               | `StringLiteral`                                          | `AttributeValue::String{value,span}` |
| Text (child)                                | Pass 1                               | `Text`                                                   | `Child::Text{value,span}` |
| AttributeValue                              | Pass 1 (revised Pass 2)              | `AttributeValue`                                         | `AttributeValue`                |
| Expression (wrapper)                        | Pass 2                               | `Expression`                                             | `Expression`                    |
| Literal (String/Number/Boolean/Null)        | Pass 2                               | `Literal`                                                | `Literal`                       |
| Reference                                   | Pass 2                               | `Reference`                                              | `Expression::Reference{name,span}` |
| MemberAccess                                | Pass 2                               | `MemberAccess`                                           | `Expression::MemberAccess{..}`  |
| Child (Text \| Expression)                  | Pass 2                               | *extends existing `Vec<Text>` to a child enum — see §11* | same                            |
| UnaryExpression                             | Pass 3a                              | `UnaryExpression` (+ `UnaryOperator`)                     | `Expression::Unary{operator,operand}` (reuses `mesh_syntax::UnaryOperator`) |
| BinaryExpression                            | Pass 3a                              | `BinaryExpression` (+ `BinaryOperator`)                   | `Expression::Binary{operator,left,right}` (reuses `mesh_syntax::BinaryOperator`) |
| ConditionalExpression                       | Pass 3a                              | `ConditionalExpression`                                  | `Expression::Conditional{condition,consequent,alternate}` |
| ArrayExpression                             | Pass 3b                              | `ArrayExpression`                                        | `Expression::Array{elements,span}` |
| ObjectExpression                            | Pass 3b                              | `ObjectExpression` (+ `ObjectMember`, `ObjectKey`)        | `Expression::Object{members,span}` (IR-local `ObjectMember{key, key_span, value, span}` — `ObjectKey` flattens, see note below) |
| CommandInvocation                           | Pass 3b                              | `CommandInvocation`                                      | `Expression::Command{command,arguments}` |
| EventValue                                  | Pass 3b (alongside CommandInvocation) | `EventValue`                                             | `Expression::EventValue{name,span}` |
| EventBinding                                | Pass 4b                              | `EventBinding`                                            | `EventBinding`                  |
| Child::Element (nested)                     | Pass 4a                              | extends `Child` enum                                     | same                            |
| Diagnostics (plural, from both parse+lower) | Pass 4a                              | n/a — changes `parse`/`lower` signatures                 | n/a                             |

Pass 4a's and Pass 4b's Rust type shapes are fixed above too, for the
same reason Pass 3a's and Pass 3b's were:
`docs/superpowers/specs/2026-09-23-mesh-v0.1-pass-4-design.md`'s
brainstorm happened immediately ahead of both passes' `writing-plans`
cycles, so there was no gap between deciding and implementing to leave
open. This spec still fixes only *grammar and semantics*, not Rust API
signatures, as a general rule — a future pass without that immediate
brainstorm-to-plan adjacency should leave its own Rust shapes open at
spec time, same as the original reasoning intended.

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

**Spans in the Semantic IR (v0.2 Pass 2):** the v0.1 IR mirrored the AST *without* spans. From v0.2 every IR node carries a `span` (a `mesh_syntax::Span`, byte offsets), and the parts of a construct that a diagnostic can point at on their own carry a separate span: `name_span` on elements, attributes and event bindings, `property_span` on member access, `command_span` on commands, and `key_span` on object members. The AST carries the same name spans. Tuple variants that had to gain a span became struct variants; `Literal` stays span-free, with its span on `Expression::Literal`.

---

## 9. Checking against a component model

*Added in v0.2.* This section is the static semantics of MPRX against a **component manifest** (its format is in the [manifest reference](./manual/manifest.md)). Without a manifest none of it applies: a file is checked exactly as §2–§8 describe, as in v0.1. With one, a file that parses is also checked as below. A file with a syntax error isn't.

Everything here is an **error**, and nothing is checked more loosely to let a file through: generated UI fails closed. `any` and `any?` (§9.2) are the only deliberate escape hatches. Each rule names the diagnostic code it raises; the [diagnostics reference](./manual/diagnostics.md#model-errors) has every message.

### 9.1 Templates, instances and scope

- **A file is the template of one manifest component**, which the compiler is given; the CLI takes it from `--component`, or from the file's name. It is a configuration error (`manifest-missing-component`) if the manifest doesn't declare it.
- **Every element, the root included, is an instance of a manifest component** named by its tag (`unknown-component`). There are no built-in elements. An element of an unknown component has its attributes' and handlers' expressions checked (they belong to the template), but not as props or events.
- **Attributes are the instance's props** (`unknown-prop`), and **`on.` bindings its events** (`unknown-event`). Every prop declared required must be written (`missing-required-prop`). Duplicate attributes and bindings are resolved first, as §3 says: only the last occurrence is checked.
- **References resolve against the template's scope and nothing else** (`unknown-reference`). The component's own props aren't in scope unless the scope lists them, and there are no global or implicit names.
- **Commands resolve against the template's commands** (`unknown-command`), and an invocation passes exactly as many arguments as the command has parameters (`command-arity-mismatch`).
- Props, events, commands and scope names are separate namespaces; each is written in its own position (`name=`, `on.name=`, `name(...)`, a bare `name`).
- Element children aren't checked against the component: a component can't declare what content it accepts. Expressions in content are still resolved and typed (§9.4), with no expected type.

### 9.2 Types

| Type | Values |
|---|---|
| `string`, `number`, `boolean` | primitive values |
| `null` | the value `null`: an ordinary value with its own type, **not** absence |
| `any` | any **present** value; it doesn't include absence |
| `list<T>` | lists whose elements are all `T` |
| records, `{ f: T, g?: U }` | objects with exactly the declared fields; each field has a type and its own requiredness |
| named types | exactly their definition in the manifest's `types` (aliases: compatibility is structural, never nominal) |
| `T?` | a `T`, **or absent**: no value at all |

Two more types exist only inside the checker and can't be written in a manifest: `void`, the type of a command invocation, which fits nowhere, not even `any`; and `nothing`, the element type of `[]`, which fits everywhere.

**Absence** is the lack of a value. `T?` means "`T`, or absent", not "`T` or `null`". MPRX has no literal for absence and no presence test, so an absent value can only come from the manifest (a `T?` scope name, field or payload, or a field that isn't required), and can only go where `T?` (or `any?`) is accepted. How a runtime represents absence is up to it; it must not be `null`.

**Requiredness** belongs to a prop's or field's declaration, not to its type, and is independent of absence:

| Declaration | May be omitted? | If written, the value must fit |
|---|---|---|
| `p: T` | no | `T` |
| `p?: T` | yes | `T` |
| `p: T?` | no | `T?` |
| `p?: T?` | yes | `T?` |

**Requiredness is erased on read.** Reading a record field gives the type of the values that may be observed: `f?: T`, `f: T?` and `f?: T?` all read as `T?` (and a field whose type is already optional doesn't become doubly optional), and only `f: T` reads as `T`. Requiredness matters in exactly two places: required props must be written, and record declarations are compared field by field (rule 8 below).

### 9.3 Compatibility

**Every check that a value fits somewhere uses one relation, `is_assignable(actual, expected)`.** Named types are expanded first, and the first rule that matches decides:

1. Either side is `void`: **no**.
2. Optionality, for every type, `any` included:
   - `expected` is `U?`: if `actual` is `T?`, the answer is `is_assignable(T, U)`; otherwise it is `is_assignable(actual, U)`. A present value satisfies "may be absent".
   - `expected` isn't optional and `actual` is `T?`: **no**, because the value may be absent.
3. `expected` is `any`: **yes**.
4. `actual` is `any`: **yes**. `any` is unchecked against the value's type, never against absence.
5. `actual` is `nothing`: **yes**.
6. Both are primitive (`string`, `number`, `boolean`, `null`): **yes** if they are the same type. There are no coercions.
7. `list<T>` to `list<U>`: `is_assignable(T, U)`. Lists are covariant, which is sound because MPRX values are never mutated.
8. Record to record, comparing **declarations**: yes if every field `expected` declares required is declared required in `actual` with an assignable type; every field `expected` declares optional has an assignable type if `actual` declares it at all; and `actual` declares no field `expected` lacks. So `f?: T` and `f: T?` are not interchangeable, either way round.
9. Anything else: **no**.

So `any` fits `T`, `any?` doesn't fit `T` but fits `T?`, and `T?` doesn't fit `any` but fits `any?`. Records are exact: there is no width subtyping.

**The common type** of two types, `join(A, B)`, is used where several values must have one type with nothing expected (§9.4). Named types are expanded first, and the first rule that matches decides:

1. Either is `void`: none.
2. Optionality: `join(T?, U)`, `join(T, U?)` and `join(T?, U?)` are all `join(T, U)?`, if `join(T, U)` exists. Optionality is never discarded, so `join(any, T?)` is `any?`.
3. `join(nothing, X)` and `join(X, nothing)` are `X`.
4. `join(any, X)` and `join(X, any)` are `any`.
5. Structurally identical types: that type. Records are identical only if they declare the same fields, each with the same requiredness and an identical type.
6. `join(list<T>, list<U>)` is `list<join(T, U)>`.
7. Anything else: none.

`join` never widens across absence, `null` or `void`, and never invents compatibility `is_assignable` would deny: both types are assignable to their join.

### 9.4 Expression types

Every expression has a type, computed bottom-up. "Needs `T`" means `is_assignable(operand, T)` (§9.3): `any` is accepted, and any optional type, `any?` included, is rejected. A failure is `type-mismatch` at the operand.

| Expression | Type |
|---|---|
| string, number, `true`/`false`, `null` literal | `string`, `number`, `boolean`, `null` |
| a quoted attribute value, `title="Users"` | `string` |
| reference `x` | its scope type |
| member access `a.f` | see below |
| `!x` | needs `boolean`; gives `boolean` |
| `-x` | needs `number`; gives `number` |
| `x + y`, `x - y`, `x * y`, `x / y`, `x % y` | both need `number`; gives `number` |
| `x < y`, `x <= y`, `x > y`, `x >= y` | both need `number`; gives `boolean` |
| `x && y`, `x \|\| y` | both need `boolean`; gives `boolean` |
| `x == y`, `x != y` | `join(x, y)` must exist; gives `boolean` |
| `c ? a : b` | `c` needs `boolean`; gives `join(a, b)` |
| `[]` | `list<nothing>` |
| `[a, b, ...]` | `list<join of all elements>` |
| `{ k: v, ... }` | an exact record with every key declared required, each typed by its value |
| a command invocation | `void` (§9.5) |
| `$event` | the event's payload type (§9.5) |

- **`+` is numeric only.** There is no string concatenation: text content already interleaves text and expressions.
- **Member access `a.f`:** on a record, the field's type as read (§9.2), or `unknown-member` at `f` if there is no such field; on `any`, `any`; on `T?` (`any?` included), `possibly-absent-access` at `a`, since MPRX has no optional chaining; on anything else (primitives, lists, `null`), `unknown-member` at `f`: there are no built-in members.
- **Common types:** equality, a conditional's branches and an array's elements with no common type are `no-common-type`: at the whole comparison, at the whole conditional, or at the first element that has none with the elements before it.
- **Object keys:** a repeated key is `duplicate-object-key` at each earlier occurrence, and the last occurrence counts, as for attributes (§3). An earlier occurrence's value is still typed, but isn't part of the object's type.

**Evaluation.** Expressions are side-effect free, so evaluation order never changes a result, but runtimes must agree on it:

- `&&` and `||` evaluate left to right and **short-circuit**: the right operand is evaluated only when the left doesn't decide the result. Their result is a `boolean`, never one of the operands. Both operands are always type-checked, and nothing is narrowed by evaluation order.
- `c ? a : b` evaluates only the chosen branch. Both are type-checked.
- `==` and `!=` are **strict**: no coercion. Present values compare structurally: primitives by value, lists element by element, records field by field. A value of type `any` compares by its actual value, still without coercion. When either side may be absent, absent equals absent, and absent never equals a present value. Equality is not a presence test: its result never narrows a type, so a `T?` stays a `T?` after any comparison.

**Errors don't cascade.** An expression with an error, or with an error inside it, has no type, and nothing that depends on its type is checked, so one mistake gets one diagnostic: `{usr.name + 1}` is one `unknown-reference`. An operator still has its own result type (`!x` is a `boolean` whatever `x` is), so an independent mistake around it is still reported.

### 9.5 Commands and `$event`

**Commands are actions, not values.** A command invocation has type `void`. It is valid **only** as the complete handler of an `on.` binding, which was already its documented use (§6):

- a command anywhere else, such as a prop value, an operand, an argument or content, is `command-outside-handler`, at the whole invocation;
- a handler that isn't a command invocation is `handler-not-command`.

Neither is looked inside: the construct is misplaced as a whole.

**`$event`** is the value the handled event carries:

- It may appear only inside the arguments of a handler's command, at any depth (`save({ user: $event })`). Elsewhere it is `event-value-outside-handler`.
- Its type is the payload of the handled event, as declared by the component of the element the `on.` sits on. An event declared without a payload has no `$event`: using it is `event-has-no-payload`.
- Any other `$` name is `unknown-special-value`.
- `$event` is never a scope name.

### 9.6 Checking values against their declarations

Each of these values is checked with `is_assignable` (§9.3) against its declared type; a failure is `type-mismatch` at the value:

- the value of a prop, where a quoted attribute is a `string`;
- each argument of a command that resolved and was given the right number of arguments, `$event` included.

An unknown prop's value, and the arguments of an unknown command or of one given the wrong number, are typed but not checked: the name or the count is the mistake.

**Literals meet their expected type part by part.** Where an expected type is known, a literal is checked against it piece by piece, and each mistake is reported where it is, instead of as one mismatch over the whole value. The relation is the same; only the reporting is finer:

- An **object literal** where a record is expected (after rule 2 strips an optional: a literal is always present) is checked field by field: a key the record doesn't declare is `unknown-field` at the key; each value is checked against its field's declared type, recursively; each required field that's missing is `missing-required-field` at the literal.
- An **array literal** where a list is expected (after rule 2 strips an optional, as for a record) checks each element against the list's element type, so its elements need no common type. Only lists are pushed into arrays: an array where `any` is expected is typed as §9.4 says, so `[1, "a"]` is still `no-common-type` there.
- A **conditional** where any type is expected checks each branch against that type, so its branches need no common type. (So `flag ? 1 : "a"` fits `any`: whichever branch runs gives a present value.)

Anywhere else, such as an object literal where `any` or a primitive is expected, the literal is typed as §9.4 says and compared whole.

---

## 10. Explicitly out of scope

- Comments (§2)
- Array/object subscript access (§5)
- Optional chaining (§7), and any other presence test for a value that
  may be absent (§9.2)
- Slots, component composition beyond flat props (`docs/ARCHITECTURE.md`
  §10 calls these out as eventual), and so checking element children
  against a component (§9.1)
- Type annotations in MPRX, and types beyond §9.2: unions, nullable types
  separate from absence, generics beyond `list<T>`, function types, open
  records, implicit coercions, and narrowing

---

## 11. Revision history

*For contributors: how each revision of this spec changed the implementation plans. You can skip this section if you just want to write MPRX.*

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
  outline's cross-cutting risk assessment for Pass 4a (children model
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
- **v0.2 (2026-09-24)** adds §9, the static semantics against a component
  manifest, as settled by `docs/superpowers/specs/2026-09-24-mesh-v0.2-outline.md`
  (D3–D5, D8–D10 and D15) and implemented by the Passes 4–5 plan. It
  changes no syntax. The out-of-scope list becomes §10 and loses
  component-model checking, and this history becomes §11.
