# Writing MPRX

This guide walks through everything MPRX v0.1 can express, with runnable examples. Every snippet here passes `mesh check` unless it's marked as a mistake. For the formal grammar, see the [MPRX Language Spec](../MPRX-SPEC.md).

## The shape of a document

An MPRX file holds **exactly one root element**. Everything else nests inside it.

```xml
<page title="Users">
  <heading>Users</heading>
  <user-list users={users} />
</page>
```

A second top-level element, or an empty file, is a syntax error.

## Elements

Tags work as they do in HTML and JSX. An element either closes itself or wraps children:

```xml
<avatar src={user.avatar} />
<text>Hello</text>
<list></list>
```

**Tag names** start with a letter or `_` and may contain letters, digits, `_`, and hyphens (`user-card`, `nav-bar`). The name in the closing tag must match the opening tag exactly, including case.

## Attributes

An attribute is a name, `=`, and a value. The value is either a **string** or an **expression** in braces:

```xml
<avatar alt="Profile photo" size={compact ? "sm" : "md"} />
```

**Attribute names can't contain hyphens.** Hyphens are only allowed in tag names, because inside expressions `a-b` means subtraction. Use camelCase or `_` instead:

```xml
<!-- mistake: syntax error -->
<item data-id="1" />

<!-- fine -->
<item dataId="1" />
```

Strings support four escapes: `\"`, `\\`, `\n`, and `\t`.

```xml
<text title="She said \"hi\"">Quote</text>
```

If the same attribute appears twice on one element, the **last one wins** and MESH prints a warning about the earlier one.

## Text and expressions in content

Children can be text, expressions, or other elements, mixed in any order:

```xml
<text>Signed in as {user.name} ({user.role})</text>
```

Text runs until the next `<` or `{`. To show either character literally, put it in a string expression:

```xml
<text>{"a < b"} and {"{braces}"}</text>
```

Text is kept exactly as written, with no trimming or collapsing. Whitespace-only text between tags, such as indentation and newlines, is treated as formatting and is left out of the compiled output.

## Expressions

Expressions go inside `{...}`. They **read and combine values but never do anything**: expressions have no side effects.

| Kind | Examples |
|---|---|
| Literals | `"text"`, `42`, `3.5`, `true`, `false`, `null` |
| References | `user`, `layout` |
| Member access | `user.name`, `items.length` |
| Unary | `!user.active`, `-count` |
| Arithmetic | `a + b`, `a - b`, `a * b`, `a / b`, `a % b` |
| Comparison | `a < b`, `a <= b`, `a > b`, `a >= b`, `a == b`, `a != b` |
| Logic | `a && b`, `a \|\| b` |
| Conditional | `compact ? "sm" : "md"` |
| Grouping | `(a + b) * c` |
| Arrays | `["a", "b"]` |
| Objects | `{ color: "red", "font-size": 12 }` |
| Commands | `selectUser($event)`, `format(user.name)` |
| Event value | `$event` |

Operators follow the usual C/JavaScript precedence. From tightest to loosest: member access, then unary, `* / %`, `+ -`, comparisons, `== !=`, `&&`, `||`, and finally `? :`. Use parentheses when in doubt.

Here's most of it in one element:

```xml
<panel
  count={items.length + 1}
  label={"Hello, " + user.name}
  visible={user.active && !user.banned}
  size={compact ? "sm" : "md"}
  tags={["a", "b",]}
  style={{ color: "red", "font-size": 12 }}
  empty={null}
  on.submit={save(form, $event)} />
```

A few details worth knowing:

- **Objects need double braces** as attribute or child values. The outer `{}` marks "this is an expression" and the inner `{}` is the object itself, just as in JSX. `data={ k: "v" }` is a syntax error; write `data={{ k: "v" }}`.
- **Object keys** are identifiers (`color`) or strings (`"font-size"`). Use a string for a key with a hyphen.
- **Trailing commas** are allowed in arrays and objects, but not in command arguments. `f(a, b,)` is a syntax error.
- **Negative numbers** are just unary minus applied to a number, so `-3.5` and `-(a + b)` both work.

## Event bindings

An event binding says what should happen when an element emits an event:

```xml
<button on.click={save()}>Save</button>
<user-card user={user} on.select={selectUser($event)} />
```

The syntax is `on.<event>={expression}`. The handler is usually a **command invocation**, which is a *request* by name plus arguments. MESH records it; the runtime (NEXUS) decides what the command does. `$event` refers to the value the event carries.

Any event name is accepted. v0.1 doesn't keep a registry of known events and doesn't check handler types. An attribute named plain `on` (`on="x"`) is still an ordinary attribute.

As with attributes, if the same event is bound twice on one element, the last binding wins and MESH warns about the earlier one.

## What MPRX deliberately doesn't have

These are syntax errors in v0.1. Most of them are left out on purpose, to keep the language small enough to check completely:

| Not supported | Instead |
|---|---|
| Comments (`<!-- -->`) | none in v0.1 |
| Subscripts (`items[0]`, `map["key"]`) | expose the value you need as a named property |
| Optional chaining (`user?.name`) | none in v0.1 |
| Hyphenated attribute names (`data-id`) | `dataId` or `data_id` |
| Arbitrary code, assignments, function definitions | commands: `doThing(args)` |

Also out of scope for v0.1: checking references against a component model. `mesh check` verifies structure, so `<user-card user={usr} />` passes even if nothing named `usr` exists. Component-aware type checking is the next milestone. v0.2 adds `mesh check --model`, which loads a component manifest; checking the file against it is still to come.

## Common mistakes at a glance

| You wrote | What happens | Fix |
|---|---|---|
| `<item>Two</itme>` | `error[mismatched-closing-tag]` | Make the closing tag match |
| `<a /><b />` at top level | `error[syntax-error]` | Wrap them in one root element |
| `data={ k: "v" }` | `error[single-brace-object]` | `data={{ k: "v" }}` |
| `<text>a < b</text>` | `error[less-than-in-text]` | `<text>{"a < b"}</text>` |
| `data-id="1"` | `error[hyphenated-attribute-name]` | `dataId="1"` |
| `label="A" label="B"` | `warning[duplicate-attribute]` | Remove the one you don't mean |

Every diagnostic has a code in brackets, like `less-than-in-text`. Look it up in the [diagnostics reference](../manual/diagnostics.md) for the full explanation and fix.
