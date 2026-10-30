// The screen: the Tier 1 helper that keeps the render on screen, releases the one before it, holds the
// unit occurrences' state and runs their transitions, so a host only shows snapshots and dispatches events.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { init, openScreen, render } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { applyPatches } from "../dist/patches.js";
import { template, testModule } from "./common.mjs";

await init(readFileSync(testModule));
const retained = () => instanceForTests().mesh_retained_renders();
const settle = () => new Promise((resolve) => setTimeout(resolve, 0)); // a release queues behind the calls made

const MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    box: { props: {}, events: {}, commands: {}, scope: {} },
    text: { props: {}, events: {}, commands: {}, scope: {} },
    button: { props: {}, events: { click: {} }, commands: {}, scope: {} },
    disclosure: {
      props: { label: { type: { kind: "string" }, required: true } },
      events: {},
      commands: {},
      scope: { label: { kind: "string" } },
      unit: { state: { open: { type: { kind: "boolean" }, default: false } }, commands: { toggle: { parameters: [] } } },
    },
    tabs: {
      props: { at: { type: { kind: "number" }, required: false } },
      events: {},
      commands: {},
      scope: {},
      unit: { state: { at: { type: { kind: "number" }, default: 0, controllable: true } }, commands: { select: { parameters: [{ name: "n", type: { kind: "number" } }] } } },
    },
    page: {
      props: {},
      events: {},
      commands: { refresh: { parameters: [] } },
      scope: {
        items: { kind: "list", element: { kind: "record", fields: { id: { type: { kind: "number" }, required: true }, label: { type: { kind: "string" }, required: true } } } },
      },
    },
  },
});
const PAGE = "<box><button on.click={refresh()}>refresh</button><box>{#each items as item (item.id)}<disclosure label={item.label} />{/each}</box><tabs /><tabs at={4} /></box>";
const DISCLOSURE = "<box><button on.click={toggle()}>{label}</button>{#if open}<text>open {label}</text>{/if}</box>";
const TABS = "<box><button on.click={select(at + 1)}>at {at}</button></box>";
const program = {
  root: "page",
  templates: [await template("page", PAGE, MODEL), await template("disclosure", DISCLOSURE, MODEL), await template("tabs", TABS, MODEL)],
};
const items = (...ids) => ({ items: ids.map((id) => ({ id, label: String.fromCharCode(96 + id) })) });
const transitions = {
  disclosure: { toggle: (state) => ({ open: !state.open }) },
  tabs: { select: (_state, n) => ({ at: n }) },
};

function texts(tree) {
  const out = [];
  const walk = (node) => {
    for (const child of node.children) {
      if (child.type === "text") out.push(child.text);
      else walk(child);
    }
  };
  walk(tree.root);
  return out;
}

/** The handler of the `index`th button in the tree, in document order. */
function button(tree, index) {
  const found = [];
  const walk = (node) => {
    if (node.component === "button") found.push(node.events.click);
    for (const child of node.children) if (child.type === "node") walk(child);
  };
  walk(tree.root);
  return found[index];
}

/** A renderer: draws the first tree, then applies the patches it is given. */
function drawing() {
  const drawn = { tree: undefined };
  drawn.take = (shown) => {
    assert.equal(shown.diagnostics, undefined, JSON.stringify(shown.diagnostics));
    drawn.tree = shown.tree === undefined ? applyPatches(drawn.tree, shown.patches) : shown.tree;
  };
  return drawn;
}

test("show gives the first tree, then patches, and keeps only the render on screen in the module", async () => {
  const screen = openScreen({ program, model: MODEL });
  const drawn = drawing();
  const first = await screen.show(items(1, 2));

  assert.ok(first.tree, "the first show gives a tree");
  assert.equal(first.patches, undefined);
  drawn.take(first);
  for (const ids of [[1, 2, 3], [3, 1], [3, 1, 2]]) {
    const next = await screen.show(items(...ids));

    assert.equal(next.tree, undefined, "a later show gives patches, not a tree");
    drawn.take(next);
    assert.deepEqual(drawn.tree, (await render({ program, model: MODEL, snapshot: items(...ids) })).render.tree);
    await settle();
    assert.equal(retained(), 1, "the render before it was released");
  }
  screen.close();
  await settle();
  assert.equal(retained(), 0, "close released the render on screen");
});

test("a show the runtime refuses changes nothing: the render on screen still dispatches", async () => {
  const screen = openScreen({ program, model: MODEL });
  const drawn = drawing();

  drawn.take(await screen.show(items(1)));
  const refused = await screen.show({ items: "nope" });

  assert.equal(refused.diagnostics.diagnostics[0].code, "runtime-value-mismatch");
  const { intent } = await screen.dispatch(button(drawn.tree, 0));

  assert.equal(intent.command.name, "refresh");
  drawn.take(await screen.show(items(1, 2)));
  assert.deepEqual(texts(drawn.tree), ["refresh", "a", "b", "at 0", "at 4"]);
  screen.close();
});

test("an application command comes back as its intent", async () => {
  const screen = openScreen({ program, model: MODEL, transitions });
  const { tree } = await screen.show(items(1));
  const result = await screen.dispatch(button(tree, 0));

  assert.deepEqual(result.intent.command, { component: "page", name: "refresh" });
  assert.equal(result.patches, undefined);
  screen.close();
});

test("a unit's command runs its transition on the occurrence's state, and the state stays with the occurrence", async () => {
  const screen = openScreen({ program, model: MODEL, transitions });
  const drawn = drawing();

  drawn.take(await screen.show(items(1, 2)));
  const toggled = await screen.dispatch(button(drawn.tree, 2)); // b's disclosure

  assert.equal(toggled.intent, undefined, "the screen ran it: there is nothing for the host to do");
  assert.equal(typeof toggled.occurrence, "string");
  drawn.take(toggled);
  assert.deepEqual(texts(drawn.tree), ["refresh", "a", "b", "open b", "at 0", "at 4"]);

  // A reorder keeps b's state with b.
  drawn.take(await screen.show(items(2, 1)));
  assert.deepEqual(texts(drawn.tree), ["refresh", "b", "open b", "a", "at 0", "at 4"]);

  // b leaves the render, so its occurrence ends; when it comes back it is a new one, with the defaults.
  drawn.take(await screen.show(items(1)));
  drawn.take(await screen.show(items(1, 2)));
  assert.deepEqual(texts(drawn.tree), ["refresh", "a", "b", "at 0", "at 4"]);
  await settle();
  assert.equal(retained(), 1);
  screen.close();
});

test("a transition's arguments are the intent's, and a field the caller owns is not changed", async () => {
  const screen = openScreen({ program, model: MODEL, transitions });
  const drawn = drawing();

  drawn.take(await screen.show(items()));
  drawn.take(await screen.dispatch(button(drawn.tree, 1))); // the free tabs: select(0 + 1)
  assert.deepEqual(texts(drawn.tree), ["refresh", "at 1", "at 4"]);

  const owned = await screen.dispatch(button(drawn.tree, 2)); // the controlled tabs: at={4}

  drawn.take(owned);
  assert.deepEqual(owned.patches.patches, [], "the caller owns `at`: nothing changed");
  assert.deepEqual(texts(drawn.tree), ["refresh", "at 1", "at 4"]);
  screen.close();
});

test("a unit's command with no transition comes back as its intent, for the host", async () => {
  const screen = openScreen({ program, model: MODEL });
  const { tree } = await screen.show(items(1));
  const { intent } = await screen.dispatch(button(tree, 1));

  assert.equal(intent.command.name, "toggle");
  assert.equal(typeof intent.instance, "string");
  screen.close();
});

test("calls are taken in order, so a show not awaited still patches from the one before it", async () => {
  const screen = openScreen({ program, model: MODEL });
  const drawn = drawing();
  const shown = [screen.show(items(1)), screen.show(items(1, 2)), screen.show(items(2))];

  for (const each of shown) drawn.take(await each);
  assert.deepEqual(texts(drawn.tree), ["refresh", "b", "at 0", "at 4"]);
  screen.close();
});

test("close ends what is shown: the next show gives a tree again, and dispatch needs one", async () => {
  const screen = openScreen({ program, model: MODEL });

  await screen.show(items(1));
  await screen.show(items(1, 2));
  screen.close();
  screen.close(); // safe twice
  await assert.rejects(screen.dispatch("h-any"), { name: "MeshUsageError", code: "invalid-argument" });
  assert.ok((await screen.show(items(1))).tree, "after close, a show starts again");
  screen[Symbol.dispose]();
  await settle();
  assert.equal(retained(), 0);
});

test("a show in flight when close is called leaves nothing held", async () => {
  const screen = openScreen({ program, model: MODEL });

  await screen.show(items(1));
  const pending = screen.show(items(1, 2));

  screen.close();
  await pending;
  await settle();
  assert.equal(retained(), 0);
  assert.ok((await screen.show(items(1))).tree, "the screen shows nothing, so the next show is a tree");
  screen.close();
});

test("openScreen checks its arguments", () => {
  assert.throws(() => openScreen({ program, model: 1 }), { name: "MeshUsageError", code: "invalid-argument" });
  assert.throws(() => openScreen({ program, model: MODEL, transitions: { disclosure: { toggle: 1 } } }), { code: "invalid-argument" });
});
