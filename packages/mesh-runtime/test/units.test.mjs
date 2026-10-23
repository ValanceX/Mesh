// Units: the state of one occurrence of a composite, held by the host and given to the runtime
// beside the snapshot. The JavaScript package encodes it, hands it to the module and reads back
// the occurrences; it judges nothing (the Rust runtime's tests are the judge).
import assert from "node:assert/strict";
import { test } from "node:test";
import { dispatch, raise, render, update, updateChanges, diff } from "../dist/index.js";
import { template } from "./common.mjs";

const MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    box: { props: {}, events: {}, commands: {}, scope: {} },
    text: { props: {}, events: {}, commands: {}, scope: {} },
    button: { props: {}, events: { click: {} }, commands: {}, scope: {} },
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
    disclosure: {
      props: { label: { type: { kind: "string" }, required: true } },
      events: {},
      commands: {},
      scope: { label: { kind: "string" } },
      unit: { state: { open: { type: { kind: "boolean" }, default: false } }, commands: { toggle: { parameters: [] } } },
    },
    tabs: {
      props: { at: { type: { kind: "number" }, required: false } },
      events: { change: { payload: { kind: "number" } } },
      commands: {},
      scope: {},
      unit: { state: { at: { type: { kind: "number" }, default: 0, controllable: true } }, commands: { select: { parameters: [{ name: "n", type: { kind: "number" } }] } } },
    },
    page: {
      props: {},
      events: {},
      commands: { chose: { parameters: [{ name: "n", type: { kind: "number" } }] } },
      scope: {
        items: { kind: "list", element: { kind: "record", fields: { id: { type: { kind: "number" }, required: true }, label: { type: { kind: "string" }, required: true } } } },
      },
    },
  },
});
const PAGE = "<box><box>{#each items as item (item.id)}<disclosure label={item.label} />{/each}</box><tabs on.change={chose($event)} /></box>";
const DISCLOSURE = "<box><button on.click={toggle()}>{label}</button>{#if open}<text>open {label}</text>{/if}</box>";
const TABS = "<box><button on.click={select(at + 1)}>at {at}</button></box>";
const program = {
  root: "page",
  templates: [await template("page", PAGE, MODEL), await template("disclosure", DISCLOSURE, MODEL), await template("tabs", TABS, MODEL)],
};
const items = (...ids) => ({ items: ids.map((id) => ({ id, label: String.fromCharCode(96 + id) })) });

function texts(tree) {
  const out = [];
  const walk = (node) => {
    for (const child of node.children ?? []) {
      if (child.type === "text") out.push(child.text);
      else walk(child);
    }
  };
  walk(tree.root);
  return out;
}

test("an occurrence is listed with its defaults, and the host's state for it is what the template sees", async () => {
  const { render: plain } = await render({ program, model: MODEL, snapshot: items(1, 2) });

  assert.equal(plain.instances.length, 3);
  assert.deepEqual(plain.instances.map((each) => each.component), ["disclosure", "disclosure", "tabs"]);
  assert.deepEqual(plain.instances[0].state, { open: false });
  assert.ok(Object.isFrozen(plain.instances));
  assert.deepEqual(texts(plain.tree), ["a", "b", "at 0"]);

  const second = plain.instances[1].instance;
  const { render: open } = await render({ program, model: MODEL, snapshot: items(1, 2), units: { [second]: { open: true } } });

  assert.deepEqual(texts(open.tree), ["a", "b", "open b", "at 0"]);
});

test("a render with no unit has none listed", async () => {
  const bare = { root: "page", templates: [await template("page", "<box><text>x</text></box>", MODEL)] };
  const { render: made } = await render({ program: bare, model: MODEL, snapshot: items() });

  assert.deepEqual(made.instances, []);
});

test("a reorder keeps the state with the row, and an update rebuilds a part the unit state changed under", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: items(1, 2, 3) });
  const b = first.instances[1].instance;
  const units = { [b]: { open: true } };
  const { render: second, patches } = await update(first, items(1, 2, 3), units);

  assert.deepEqual(texts(second.tree), ["a", "b", "open b", "c", "at 0"]);
  assert.ok(patches.patches.length > 0);
  assert.equal(second.instances.length, 4);

  const { render: sorted } = await update(second, items(3, 2, 1));

  // No units given: the render's own are kept, and the row's state moved with it.
  assert.deepEqual(texts(sorted.tree), ["c", "b", "open b", "a", "at 0"]);
  assert.equal(sorted.instances.find((each) => each.state.open === true).instance, b);

  const fresh = await render({ program, model: MODEL, snapshot: items(3, 2, 1), units });

  assert.deepEqual(sorted.tree, fresh.render.tree);
  second.release();
  sorted.release();
});

test("a command of a unit names its occurrence, and so does the same dispatch of a render the module no longer holds", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: items(1, 2) });
  const { render: held } = await update(first, items(1, 2));
  const handler = held.tree.root.children[0].children[1].children[0].events.click;
  const { intent } = await dispatch(held, handler);

  assert.equal(intent.command.component, "disclosure");
  assert.equal(intent.command.name, "toggle");
  assert.equal(intent.instance, held.instances[1].instance);
  assert.equal(intent.controlled, undefined);
  held.release();

  const { intent: again } = await dispatch(held, handler);

  assert.deepEqual(again, intent);
});

test("a controlled field takes the caller's prop, and raise gives the handler the caller bound", async () => {
  const controlled = {
    root: "page",
    templates: [await template("page", "<box><tabs at={4} on.change={chose($event)} /></box>", MODEL), await template("tabs", TABS, MODEL)],
  };
  const { render: made } = await render({ program: controlled, model: MODEL, snapshot: items() });
  const [tabs] = made.instances;

  assert.deepEqual(tabs.controlled, ["at"]);
  assert.deepEqual(texts(made.tree), ["at 4"]);

  const { intent } = await raise(made, tabs.instance, "change", 2);

  assert.equal(intent.command.name, "chose");
  assert.deepEqual(intent.arguments, [{ value: 2 }]);

  const { diagnostics } = await raise(made, "k-nowhere", "change", 2);

  assert.equal(diagnostics.diagnostics[0].code, "runtime-unknown-instance");

  const quiet = {
    root: "page",
    templates: [await template("page", "<box><tabs /></box>", MODEL), await template("tabs", TABS, MODEL)],
  };
  const { render: unbound } = await render({ program: quiet, model: MODEL, snapshot: items() });
  const { intent: nothing, diagnostics: none } = await raise(unbound, unbound.instances[0].instance, "change", 2);

  assert.equal(none, undefined);
  assert.equal(nothing, undefined);
});

test("a state the manifest forbids is a diagnostic, and units of the wrong shape are a usage error", async () => {
  const { render: plain } = await render({ program, model: MODEL, snapshot: items(1) });
  const key = plain.instances[0].instance;
  const { diagnostics } = await render({ program, model: MODEL, snapshot: items(1), units: { [key]: { open: "yes" } } });

  assert.equal(diagnostics.diagnostics[0].code, "runtime-value-mismatch");
  await assert.rejects(render({ program, model: MODEL, snapshot: items(1), units: [] }), { code: "invalid-argument" });
  await assert.rejects(render({ program, model: MODEL, snapshot: items(1), units: { [key]: 1 } }), { name: "MeshUsageError" });
});

test("updateChanges keeps the units, or takes others", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: items(1, 2) });
  const key = first.instances[0].instance;
  const { render: opened } = await update(first, items(1, 2), { [key]: { open: true } });
  const changes = { base: opened.version, changes: diff(items(1, 2), items(1, 2, 3)) };
  const { render: grown } = await updateChanges(opened, changes);

  assert.deepEqual(texts(grown.tree), ["a", "open a", "b", "c", "at 0"]);

  const back = { base: grown.version, changes: [] };
  const { render: closed } = await updateChanges(grown, back, { units: {} });

  assert.deepEqual(texts(closed.tree), ["a", "b", "c", "at 0"]);
});
