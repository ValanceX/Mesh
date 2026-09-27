# Rendering MESH output

This guide is for whoever builds a renderer for MESH's output, such as one of PORT's: the code that draws a render tree on a screen, updates it when a new tree arrives, and reports what the user does. The renderers live in the PORT repository, not in MESH. MESH gives them the render tree, whose types `@valancex/mesh-runtime` exports. This guide says what a renderer may and must do with a tree, and ends with a small working renderer.

**You'll need:** a render tree, from a host that renders with `@valancex/mesh-runtime` (see [Integrating MESH with NEXUS](./integrating-mesh-with-nexus.md)). A renderer imports only the tree's types, never the runtime.

## The render tree

A render tree ([`schemas/render-v1.schema.json`](../../schemas/render-v1.schema.json)) is a root **node**. Each node has:
- `component`: a primitive component's name, such as `avatar`. The components a program has templates for, its composites, are already expanded, and never appear;
- `props`: each prop's value: `null`, a boolean, a finite number, a string, a list or a record. It's fully evaluated, but it's the value, not text: a number prop is a number. An unwritten or absent prop has no entry;
- `propText`, when there's any: the MESH text of each prop that is a number, a boolean or `null` (`"42"`, `"false"`, `"null"`). Strings don't need one, and lists and records have none;
- `events`: each event the node listens for, with its **handler identifier**;
- `children`: nodes, and **text runs**, each a string to show as it is;
- `key`: the node's identity. Text runs have keys too.

That's everything. A tree holds no expression, no scope, no command and no composite name, so there's nothing in it to evaluate.

## Draw what you're given

**Values are final: MESH has evaluated them, and made every text.** A **text run** is already text: MESH turned each number in it into text by its own rule (§9.7.7.1), which a JavaScript engine's `String(x)` or a platform's formatter may not match in the last digit. A **prop** is different: it arrives as its value, so a number prop is a number, not text. You decide where on your target each prop goes, and then realize it in one of two ways (spec §9.8.7):

- **natively,** into a slot that holds a value of the same kind exactly: a number into a numeric property, a boolean into a boolean property or a presence, a list or record into a structure of the same shape;
- **as its MESH text,** into a slot that holds only text: a string prop's value, or the prop's `propText` entry, as given.

Nothing else. Never make text from a value yourself: not with `String(x)`, a template literal, `JSON.stringify`, or the platform's own conversion (`setAttribute(name, 42)` converts for you, and so is out). A `null` prop's text is `null`, and a `null` prop is never the same as an absent one: absent is omitted, with no text. A list or record has no text at all, so in a slot that holds only text it is unrealizable: report that as your renderer's failure, and don't invent a form for it. Don't supply a default for a missing prop, and don't convert or trim a value. If a value looks wrong, the fix is in the template or the host's values, never in the renderer.

**A component you don't know is shown, not dropped.** If a program leaves out a composite's template, that component arrives as a node of its name, like a primitive. A renderer that silently skips unknown nodes hides that mistake. Surface it: draw a visible placeholder, log it, or fail, as your platform prefers.

## Report events as handler identifiers

When the user does something, **resolve it to at most one binding** (spec §9.9), and report that binding's handler identifier, and the event's payload, to the host. Don't interpret either.

- Start at the innermost node the interaction is on: an interaction on a text run is on its node.
- If that node's primitive has an applicable event for the interaction (the one of its events the interaction constitutes, by the primitive's definition) and the node binds it, report that binding and stop.
- Otherwise move to the parent, towards the root. If no node qualifies, report nothing.

So `<card on.click={open()}><button on.click={save()}>Save</button></card>` reports only `save()`'s handler for a click on the button, and `open()`'s only for a click elsewhere on the card, or on a button with no `click` binding. This is **event resolution, not DOM bubbling**: never let a second binding fire for the same interaction, even when your target delivers the interaction to ancestors, and never relate two nodes' events because they share a name. MPRX has no way to stop or repeat resolution, and doesn't need one. Which interactions constitute a primitive's event is part of that primitive's definition, the same on every target, and never depends on the node's bindings or ancestors. `examples/conformance/events/` has language-neutral cases to test your renderer against, with no target needed. The host dispatches them with the render the tree came from, and the runtime turns them into a command intent. **A renderer never sees an intent,** or which command an event invokes.

Keys and handler identifiers are **opaque, but not secret.** Compare them only for equality, and never parse them. Anyone with the templates can compute them. What makes them safe to hand back is the runtime's validation when the host dispatches, not their secrecy.

## Updating by key

A change of values is a new render: the runtime evaluates the whole program again and gives a complete new tree. It doesn't say what changed; finding that is the renderer's job.

**Every tree from one program has the same keys.** So a renderer matches the new tree against the one it drew, key by key, and updates only the props and text that differ. Within one program that never happens; if it ever does, treat a key in only the new tree as new, and a key in only the old one as gone.

**Keys change when the program does,** by design. The host tells the renderer when a tree comes from a different program (a template recompiled, added or removed), and the renderer draws that tree afresh, never matching it against the old one by key.

## A renderer, end to end

This renderer draws the slice, `examples/slice/` in the MESH repository, as text for inspection, then updates it to the slice's second snapshot, in which the first user's name and avatar change. The first half is only there to get two trees; a PORT renderer receives them from its host.

```js
import { readFileSync } from "node:fs";
import { compile } from "@valancex/mesh-compiler";
import { render } from "@valancex/mesh-runtime";

// The host's side: two renders of the slice, one program, two snapshots.
const read = (name) => readFileSync(`examples/slice/${name}`, "utf8");
const model = read("components.json");
const templates = [];
for (const component of ["users", "user-card"]) {
  const result = await compile({
    source: read(`${component}.mprx`),
    path: `${component}.mprx`,
    model: { manifest: model, path: "components.json", component },
  });
  templates.push(JSON.stringify(result.template));
}
const program = { root: "users", templates };
const tree = async (snapshot) =>
  (await render({ program, model, snapshot: JSON.parse(read(`snapshots/${snapshot}.json`)) })).render.tree;
const first = await tree("first");
const second = await tree("second");

// The renderer's side, which sees only trees.
const KNOWN = new Set(["page", "text", "avatar", "button"]);

// Draws a tree as text: props and text as given, nothing computed.
function draw(node, indent = "") {
  if (!KNOWN.has(node.component)) {
    return `${indent}[unknown component: ${node.component}]\n`;
  }
  const props = Object.entries(node.props).map(([name, value]) => ` ${name}=${JSON.stringify(value)}`);
  const events = Object.keys(node.events).map((event) => ` on.${event}`);
  let out = `${indent}<${node.component}${props.join("")}${events.join("")}>\n`;
  for (const child of node.children) {
    out += child.type === "node" ? draw(child, `${indent}  `) : `${indent}  ${child.text}\n`;
  }
  return out;
}

// Every node and text run in a tree, by key.
function byKey(tree) {
  const parts = new Map();
  const walk = (part) => {
    parts.set(part.key, part);
    if (part.type === "node") part.children.forEach(walk);
  };
  walk(tree.root);
  return parts;
}

// What must change to turn the drawn tree into the next one, matched by key.
function updates(drawn, next) {
  const old = byKey(drawn);
  const out = [];
  for (const [key, part] of byKey(next)) {
    const was = old.get(key);
    old.delete(key);
    if (!was) {
      out.push(`add ${part.type === "node" ? `<${part.component}>` : JSON.stringify(part.text)}`);
    } else if (part.type === "text") {
      if (was.text !== part.text) out.push(`text ${JSON.stringify(was.text)} -> ${JSON.stringify(part.text)}`);
    } else {
      for (const name of new Set([...Object.keys(was.props), ...Object.keys(part.props)])) {
        const before = JSON.stringify(was.props[name]) ?? "(absent)";
        const after = JSON.stringify(part.props[name]) ?? "(absent)";
        if (before !== after) out.push(`<${part.component}> ${name}: ${before} -> ${after}`);
      }
    }
  }
  for (const part of old.values()) {
    out.push(`remove ${part.type === "node" ? `<${part.component}>` : JSON.stringify(part.text)}`);
  }
  return out;
}

process.stdout.write(draw(first.root));
console.log("--");
console.log(updates(first, second).join("\n"));
```

**Its printout is for inspection only.** It writes each prop as JSON (`size="sm"`), which is a lossless encoding for comparing trees, not MESH's text of a value, and not a model for a real renderer's output: a renderer that puts a prop in a text slot uses the prop's value if it's a string, or its `propText` entry (see [Draw what you're given](#draw-what-youre-given)). It prints the first tree, then the four updates that turn it into the second:

```text
<page title="Team">
  <page title="Ada Lovelace">
    <avatar alt="Ada Lovelace" size="sm" src="ada.png" on.click>
    <text>
      Ada Lovelace
  <page title="Grace Hopper">
    <avatar alt="Grace Hopper" size="md" on.click>
    <text>
      Grace Hopper (inactive)
  <button on.click>
    Refresh
--
<page> title: "Ada Lovelace" -> "Ada King"
<avatar> alt: "Ada Lovelace" -> "Ada King"
<avatar> src: "ada.png" -> "ada-2.png"
text "Ada Lovelace" -> "Ada King"
```

`user-card` never appears: it's a composite, expanded into the `page`, `avatar` and `text` its template draws. The second user's `avatar` has no `src`, because that user has no avatar and the prop is optional; the renderer draws nothing for it, rather than an empty string. Nothing is added or removed, because both trees come from one program.

## What a renderer doesn't do

- **Evaluate, format or default anything.** The tree's values are final, and every text it needs is in the tree.
- **Decide what an event means,** or let one interaction reach two bindings. It resolves the interaction (spec §9.9) and reports one handler identifier and payload; the host and the runtime do the rest.
- **Reconcile across programs.** Its host says when the program changed, and it draws afresh.
- **Know about NEXUS.** Intents and commands are the host's.
