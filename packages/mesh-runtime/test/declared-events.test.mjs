// The declared events of a program (`declaredEvents`), through the package and the WebAssembly module:
// the validated program's own event bindings, not a render's. The template-v1 documents are used here
// only as a test oracle; the package itself reads nothing inside the result (I11).
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkProgram } from "../../mesh-compiler/dist/index.js";
import { declaredEvents, render } from "../dist/index.js";
import { MODEL as OTHER_MODEL, template } from "./common.mjs";

const MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: { props: {}, events: {}, commands: {}, scope: {} },
    note: { props: {}, events: {}, commands: {}, scope: {} },
    row: { props: {}, events: {}, commands: {}, scope: {} },
    button: { props: {}, events: { tap: {} }, commands: {}, scope: {} },
    "mesh-if": { props: { when: { type: { kind: "boolean" }, required: true } }, events: {}, commands: {}, scope: {} },
    "mesh-each": {
      props: {
        items: { type: { kind: "list", element: { kind: "any" } }, required: true },
        as: { type: { kind: "string" }, required: true },
        key: { type: { kind: "any" }, required: true },
      },
      events: {},
      commands: {},
      scope: {},
    },
    card: { props: {}, events: {}, commands: { pick: { parameters: [] } }, scope: {} },
    simple: { props: {}, events: {}, commands: { go: { parameters: [] } }, scope: {} },
    twice: { props: {}, events: {}, commands: { go: { parameters: [] } }, scope: {} },
    users: { props: {}, events: {}, commands: { refresh: { parameters: [] } }, scope: {} },
    other: { props: {}, events: {}, commands: { reset: { parameters: [] } }, scope: {} },
    cond: { props: {}, events: {}, commands: { go: { parameters: [] } }, scope: { show: { kind: "boolean" } } },
    list: {
      props: {},
      events: {},
      commands: { open: { parameters: [{ name: "id", type: { kind: "any" } }] } },
      scope: { items: { kind: "list", element: { kind: "record", fields: { id: { type: { kind: "any" }, required: true } } } } },
    },
  },
});

const SIMPLE = "<page><button on.tap={go()}>go</button></page>";
const TWICE = "<page><button on.tap={go()}>a</button><button on.tap={go()}>b</button></page>";
const CARD = "<button on.tap={pick()}>pick</button>";
const USERS = "<page><card /><card /><button on.tap={refresh()}>refresh</button></page>";
const OTHER = "<page><card /><button on.tap={reset()}>reset</button></page>";
const COND = "<page><note>x</note><mesh-if when={show}><button on.tap={go()}>shown</button></mesh-if></page>";
const LIST =
  '<page><mesh-each items={items} as="item" key={item.id}><row><button on.tap={open(item.id)}>o</button></row></mesh-each></page>';

/** A program: its root, and each component's source; templates compiled by the real compiler. */
async function program(root, sources) {
  const templates = [];
  for (const [component, source] of sources) {
    templates.push(await template(component, source, MODEL));
  }
  return { root, templates };
}

/** The oracle: the template-v1 documents' own bindings, as sorted rows. */
function oracle(templates) {
  const rows = [];
  const walk = (component, element) => {
    for (const binding of element.events) {
      const { start, end } = binding.span;
      rows.push([component, binding.event, binding.command, start.byte, end.byte, start.utf16, end.utf16].join("|"));
    }
    for (const child of element.children) {
      if (child.kind === "element") walk(component, child.element);
    }
  };
  for (const text of templates) {
    const parsed = JSON.parse(text);
    walk(parsed.component, parsed.root);
  }
  return rows.sort();
}

/** The runtime's events as the same sorted rows. */
function rows(events) {
  return events
    .map(({ component, event, command, span: { start, end } }) =>
      [component, event, command, start.byte, end.byte, start.utf16, end.utf16].join("|"),
    )
    .sort();
}

const keys = (events) => events.map((e) => `${e.component}/${e.command}`).sort();

async function declared(program) {
  const result = await declaredEvents({ program, model: MODEL });
  assert.equal(result.diagnostics, undefined, JSON.stringify(result.diagnostics));
  return result.events;
}

function handlers(node) {
  return (
    Object.keys(node.events).length +
    node.children.reduce((count, child) => count + (child.type === "node" ? handlers(child) : 0), 0)
  );
}

test("a simple event, with its span", async () => {
  const p = await program("simple", [["simple", SIMPLE]]);
  const events = await declared(p);
  assert.equal(events.length, 1);
  const [event] = events;
  assert.deepEqual([event.component, event.event, event.command], ["simple", "tap", "go"]);
  assert.equal(SIMPLE.slice(event.span.start.byte, event.span.end.byte), "on.tap={go()}");
  assert.deepEqual(rows(events), oracle(p.templates));
});

test("a composite's event is declared once, however often the composite is used", async () => {
  const p = await program("users", [["users", USERS], ["card", CARD]]);
  const events = await declared(p);
  assert.deepEqual(keys(events), ["card/pick", "users/refresh"]);
  assert.deepEqual(rows(events), oracle(p.templates));
});

test("an event in an inactive mesh-if alternative is declared; the render has no handler for it", async () => {
  const p = await program("cond", [["cond", COND]]);
  const events = await declared(p);
  assert.deepEqual(keys(events), ["cond/go"]);
  assert.deepEqual(rows(events), oracle(p.templates));

  const hidden = await render({ program: p, model: MODEL, snapshot: { show: false } });
  const shown = await render({ program: p, model: MODEL, snapshot: { show: true } });
  assert.equal(handlers(hidden.render.tree.root), 0);
  assert.equal(handlers(shown.render.tree.root), 1);
});

test("an event in a mesh-each body is declared once, not per item", async () => {
  const p = await program("list", [["list", LIST]]);
  const events = await declared(p);
  assert.deepEqual(keys(events), ["list/open"]);
  assert.deepEqual(rows(events), oracle(p.templates));

  const three = await render({ program: p, model: MODEL, snapshot: { items: [{ id: "a" }, { id: "b" }, { id: "c" }] } });
  assert.equal(handlers(three.render.tree.root), 3);
});

test("occurrences are kept: nothing is deduplicated, and each program reports its own", async () => {
  const twice = await declared(await program("twice", [["twice", TWICE]]));
  assert.equal(twice.length, 2);
  assert.deepEqual(keys(twice), ["twice/go", "twice/go"]);
  assert.notDeepEqual(twice[0].span, twice[1].span);

  // Two programs over the same composite: `card/pick` in each.
  const users = await declared(await program("users", [["users", USERS], ["card", CARD]]));
  const other = await declared(await program("other", [["other", OTHER], ["card", CARD]]));
  assert.deepEqual(keys(users), ["card/pick", "users/refresh"]);
  assert.deepEqual(keys(other), ["card/pick", "other/reset"]);
});

test("every span is the template's own, UTF-8 bytes and UTF-16 units", async () => {
  // A multi-byte character before the binding makes the two offsets differ.
  const source = "<page><note>é</note><button on.tap={go()}>go</button></page>";
  const p = await program("simple", [["simple", source]]);
  const [event] = await declared(p);
  assert.notEqual(event.span.start.byte, event.span.start.utf16);
  assert.equal(source.slice(event.span.start.utf16, event.span.end.utf16), "on.tap={go()}");
  assert.deepEqual(rows([event]), oracle(p.templates));

  for (const [root, sources] of [
    ["users", [["users", USERS], ["card", CARD]]],
    ["cond", [["cond", COND]]],
    ["list", [["list", LIST]]],
    ["twice", [["twice", TWICE]]],
  ]) {
    const q = await program(root, sources);
    assert.deepEqual(rows(await declared(q)), oracle(q.templates), root);
  }
});

test("an invalid program gives the diagnostics the program check gives", async () => {
  const simple = await template("simple", SIMPLE, MODEL);
  const card = await template("card", CARD, MODEL);
  const invalid = [
    { root: "users", templates: [card], model: MODEL },                      // no template for the root
    { root: "simple", templates: [simple, simple], model: MODEL },           // a second template for one component
    { root: "simple", templates: [simple], model: OTHER_MODEL },            // another manifest
    { root: "simple", templates: ["not a template"], model: MODEL },        // unreadable template text
    { root: "simple", templates: [simple], model: "not a manifest" },       // an unreadable manifest
  ];
  for (const { root, templates, model } of invalid) {
    const expected = await checkProgram({ model, root, templates });
    assert.ok(expected.diagnostics.length > 0, `a program that must be invalid: ${root}`);
    const result = await declaredEvents({ program: { root, templates }, model });
    assert.equal(result.events, undefined);
    assert.deepEqual(result.diagnostics, expected, root);
  }
  // And for a missing root, as render reports it.
  const missingRoot = { root: "users", templates: [card] };
  const viaRender = await render({ program: missingRoot, model: MODEL, snapshot: {} });
  const viaDeclared = await declaredEvents({ program: missingRoot, model: MODEL });
  assert.deepEqual(viaDeclared.diagnostics, viaRender.diagnostics);
});

test("a component with no template is a primitive and declares nothing", async () => {
  const events = await declared(await program("users", [["users", USERS]]));
  assert.deepEqual(keys(events), ["users/refresh"]);
});

test("the order is by component, then document order, whatever order the templates arrive in", async () => {
  const forward = await program("users", [["users", USERS], ["card", CARD]]);
  const backward = await program("users", [["card", CARD], ["users", USERS]]);
  const a = await declared(forward);
  const b = await declared(backward);
  assert.deepEqual(a, b);
  assert.deepEqual(a.map((e) => e.component), ["card", "users"]);
  assert.deepEqual(await declared(forward), a); // and again

  const twice = await declared(await program("twice", [["twice", TWICE]]));
  assert.ok(twice[0].span.start.byte < twice[1].span.start.byte);
});

test("the result is the module's own: frozen at the envelope, plain data inside", async () => {
  const result = await declaredEvents({ program: await program("users", [["users", USERS], ["card", CARD]]), model: MODEL });
  assert.ok(Object.isFrozen(result));
  assert.deepEqual(result.events, JSON.parse(JSON.stringify(result.events)));
  for (const event of result.events) {
    assert.deepEqual(Object.keys(event).sort(), ["command", "component", "event", "span"]);
    assert.deepEqual(Object.keys(event.span).sort(), ["end", "start"]);
    assert.deepEqual(Object.keys(event.span.start).sort(), ["byte", "utf16"]);
  }
});

test("arguments of the wrong type are a TypeError", async () => {
  await assert.rejects(declaredEvents(undefined), TypeError);
  await assert.rejects(declaredEvents({ model: MODEL }), TypeError);
  await assert.rejects(declaredEvents({ program: { root: 1, templates: [] }, model: MODEL }), TypeError);
  await assert.rejects(declaredEvents({ program: { root: "a", templates: [1] }, model: MODEL }), TypeError);
  await assert.rejects(declaredEvents({ program: { root: "a", templates: [] }, model: 1 }), TypeError);
});
