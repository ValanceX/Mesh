// update(): the new render and the patches from the previous tree to its
// tree. The law (applying the patches gives exactly a full render's tree)
// is the same as the Rust runtime's, and a refused update leaves the
// previous render valid.
import assert from "node:assert/strict";
import { test } from "node:test";
import { dispatch, render, update } from "../dist/index.js";
import { template, validatePatches } from "./common.mjs";
import { generator, LIST_MODEL, listProgram, mutate } from "./common/list-program.mjs";

const MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: { props: { title: { type: { kind: "string" }, required: true } }, events: { tap: {} }, commands: {}, scope: {} },
    note: { props: {}, events: {}, commands: {}, scope: {} },
    "mesh-if": { props: { when: { type: { kind: "boolean" }, required: true } }, events: {}, commands: {}, scope: {} },
    view: {
      props: {},
      events: {},
      commands: { pick: { parameters: [] } },
      scope: { title: { kind: "string" }, who: { kind: "string" }, flag: { kind: "boolean" } },
    },
  },
});
const VIEW =
  "<page title={title} on.tap={pick()}><note>hello {who}</note><mesh-if when={flag}><note>on</note></mesh-if></page>";
const program = { root: "view", templates: [await template("view", VIEW, MODEL)] };

/** A reference applier for render-patch-v1 over a render-v1 tree. */
function apply(tree, document) {
  const out = structuredClone(tree);
  const find = (node, key) => {
    if (node.key === key) return node;
    for (const child of node.children ?? []) {
      const found = find(child, key);
      if (found) return found;
    }
    return undefined;
  };
  const parentOf = (node, key) => {
    for (const [index, child] of (node.children ?? []).entries()) {
      if (child.key === key) return [node, index];
      const found = parentOf(child, key);
      if (found) return found;
    }
    return undefined;
  };
  const place = (children, part, before) => {
    const at = before === undefined ? children.length : children.findIndex((c) => c.key === before);
    assert.ok(at >= 0, `a sibling ${before} at this point`);
    children.splice(at, 0, part);
  };
  for (const patch of document.patches) {
    if (patch.op === "replace") return structuredClone(patch.tree);
    if (patch.op === "insert") {
      const parent = find(out.root, patch.parent);
      assert.ok(parent, `a parent ${patch.parent}`);
      place(parent.children, structuredClone(patch.node), patch.before);
      continue;
    }
    if (patch.op === "remove" || patch.op === "move") {
      const [parent, index] = parentOf(out.root, patch.key);
      const [part] = parent.children.splice(index, 1);
      if (patch.op === "move") place(parent.children, part, patch.before);
      continue;
    }
    const node = find(out.root, patch.key);
    assert.ok(node, `a node ${patch.key}`);
    if (patch.op === "setText") node.text = patch.text;
    else if (patch.op === "removeProp") {
      delete node.props[patch.prop];
      if (node.propText) delete node.propText[patch.prop];
    } else if (patch.op === "setProp") {
      node.props[patch.prop] = patch.value;
      if (patch.propText !== undefined) (node.propText ??= {})[patch.prop] = patch.propText;
      else if (node.propText) delete node.propText[patch.prop];
    } else assert.fail(`unknown op ${patch.op}`);
  }
  return out;
}

const snapshot = (title, who, flag) => ({ title, who, flag });

test("an update's patches take the previous tree to the full render's", async () => {
  const states = [
    snapshot("a", "x", false),
    snapshot("b", "x", false),
    snapshot("b", "y", false),
    snapshot("b", "y", true),
    snapshot("c", "z", true),
  ];
  let { render: current } = await render({ program, model: MODEL, snapshot: states[0] });
  for (const next of states.slice(1)) {
    const updated = await update(current, next);
    assert.equal(updated.diagnostics, undefined);
    assert.ok(validatePatches(updated.patches), JSON.stringify(validatePatches.errors));
    const { render: full } = await render({ program, model: MODEL, snapshot: next });
    assert.deepEqual(updated.render.tree, full.tree);
    assert.deepEqual(apply(current.tree, updated.patches), full.tree);
    current = updated.render;
  }
});

test("a changed prop and a changed text are two small patches, and a changed structure is a remove and an insert", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: snapshot("a", "x", false) });
  const small = await update(first, snapshot("b", "y", false));
  assert.deepEqual(small.patches.patches.map((p) => p.op), ["setProp", "setText"]);
  const structural = await update(first, snapshot("a", "x", true));
  assert.deepEqual(structural.patches.patches.map((p) => p.op), ["insert"]);
  const back = await update(structural.render, snapshot("a", "x", false));
  assert.deepEqual(back.patches.patches.map((p) => p.op), ["remove"]);
});

test("a refused update gives diagnostics and leaves the previous render valid", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: snapshot("a", "x", false) });
  const refused = await update(first, { title: 1, who: "x", flag: false });
  assert.equal(refused.render, undefined);
  assert.equal(refused.diagnostics.diagnostics[0].code, "runtime-value-mismatch");
  const tap = first.tree.root.events.tap;
  const result = await dispatch(first, tap);
  assert.equal(result.intent.command.name, "pick");
});

test("update takes a Render that render() returned", async () => {
  await assert.rejects(update({}, snapshot("a", "x", false)), TypeError);
});

test("a chain of updates matches full renders, and each render's module copy is released", async () => {
  const { readFileSync } = await import("node:fs");
  const { init } = await import("../dist/index.js");
  const { instanceForTests } = await import("../dist/engine.js");
  const { testModule } = await import("./common.mjs");
  await init(readFileSync(testModule));
  const retained = () => instanceForTests().mesh_retained_renders();
  assert.equal(retained(), 0);

  const states = [snapshot("a", "x", false), snapshot("b", "x", false), snapshot("b", "y", false), snapshot("c", "y", true)];
  let { render: current } = await render({ program, model: MODEL, snapshot: states[0] });
  assert.equal(retained(), 1, "render() keeps its result in the module");
  for (const [index, next] of states.slice(1).entries()) {
    const updated = await update(current, next);
    // The new render is kept, and so is the previous one, until released.
    assert.equal(retained(), 2);
    const { render: full } = await render({ program, model: MODEL, snapshot: next });
    assert.deepEqual(updated.render.tree, full.tree);
    assert.deepEqual(apply(current.tree, updated.patches), full.tree);
    full.release();
    current.release();
    current.release(); // twice is safe
    current = updated.render;
    await new Promise((resolve) => setTimeout(resolve, 0)); // release is queued behind calls
    assert.equal(retained(), 1);
  }
  current.release();
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(retained(), 0, "released renders are gone from the module");
  // A released render still works: it derives itself again.
  const again = await update(current, snapshot("z", "z", false));
  assert.equal(again.diagnostics, undefined);
  assert.equal(again.render.tree.root.props.title, "z");
  again.render.release();
});

test("the previous render stays valid after an update, until released", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: snapshot("a", "x", false) });
  const one = await update(first, snapshot("b", "x", false));
  const two = await update(one.render, snapshot("c", "x", false));
  // `one.render` was updated from and not released: it still dispatches and still updates.
  assert.equal((await dispatch(one.render, one.render.tree.root.events.tap)).intent.command.name, "pick");
  const sibling = await update(one.render, snapshot("d", "x", false));
  assert.equal(sibling.render.tree.root.props.title, "d");
  for (const r of [one.render, two.render, sibling.render]) r.release();
});

test("a render from a module since replaced updates by deriving itself again", async () => {
  const { instanceForTests } = await import("../dist/engine.js");
  const { render: first } = await render({ program, model: MODEL, snapshot: snapshot("a", "x", false) });
  const one = await update(first, snapshot("b", "x", false));
  // Make the next call fail, so the wrapper discards this instance (and its handles).
  const failed = instanceForTests();
  failed.mesh_render_kept = () => failed.mesh_test_panic();
  await assert.rejects(render({ program, model: MODEL, snapshot: snapshot("a", "x", false) }));
  assert.notEqual(instanceForTests(), failed);
  const two = await update(one.render, snapshot("c", "x", false));
  assert.equal(two.diagnostics, undefined);
  assert.equal(two.render.tree.root.props.title, "c");
  two.render.release();
});

test("over random chains of updates, the tree the wrapper builds is the tree a full render gives", async () => {
  const below = generator(5);
  let updates = 0;
  for (let chain = 0; chain < 60; chain++) {
    let state = { title: "t", count: 0, flag: false, items: [] };
    let { render: current } = await render({ program: listProgram, model: LIST_MODEL, snapshot: state });
    for (let step = 0; step < 20; step++) {
      state = mutate(state, below);
      const updated = await update(current, state);
      assert.equal(updated.diagnostics, undefined);
      assert.ok(validatePatches(updated.patches), JSON.stringify(validatePatches.errors));
      const { render: full } = await render({ program: listProgram, model: LIST_MODEL, snapshot: state });
      assert.deepEqual(updated.render.tree, full.tree, `for ${JSON.stringify(state)}`);
      assert.ok(Object.isFrozen(updated.render.tree.root), "the new tree is frozen");
      current.release();
      current = updated.render;
      updates++;
    }
    current.release();
  }
  assert.equal(updates, 1200);
});

test("the new tree shares every part the patches didn't reach with the previous tree", async () => {
  const items = Array.from({ length: 50 }, (_, id) => ({ id, label: `l${id}`, done: false }));
  const state = { title: "t", count: 0, flag: true, items };
  const { render: first } = await render({ program: listProgram, model: LIST_MODEL, snapshot: state });
  const changed = structuredClone(state);
  changed.items[20].label = "changed";
  const updated = await update(first, changed);
  const before = first.tree.root.children;
  const after = updated.render.tree.root.children;
  assert.equal(before.length, after.length);
  const different = after.filter((child, index) => child !== before[index]).length;
  // The one row that changed, and no other part of the page.
  assert.equal(different, 1);
  assert.notEqual(updated.render.tree.root, first.tree.root);
  first.release();
  updated.render.release();
});

// Composite children and the slot, through the module and the wrapper.
const SLOT_MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: { props: {}, events: {}, commands: {}, scope: {} },
    note: { props: {}, events: {}, commands: {}, scope: {} },
    panel: { props: { heading: { type: { kind: "string" }, required: true } }, events: {}, commands: {}, scope: {} },
    "mesh-slot": { props: {}, events: {}, commands: {}, scope: {} },
    card: { props: { heading: { type: { kind: "string" }, required: true } }, events: {}, commands: {}, scope: { heading: { kind: "string" } } },
    view: { props: {}, events: {}, commands: {}, scope: { title: { kind: "string" }, who: { kind: "string" } } },
  },
});
const slotProgram = {
  root: "view",
  templates: [
    await template("view", "<page><card heading={title}><note>hello {who}</note></card><note>{title}</note></page>", SLOT_MODEL),
    await template("card", "<panel heading={heading}><note>top</note><mesh-slot /><note>end</note></panel>", SLOT_MODEL),
  ],
};

test("a composite's children are placed at its slot, and an update reaches them", async () => {
  const first = await render({ program: slotProgram, model: SLOT_MODEL, snapshot: { title: "T", who: "Ada" } });
  assert.equal(first.diagnostics, undefined);
  const text = (tree) => JSON.stringify(tree).match(/"text":"[^"]*"/g).join(" ");
  assert.equal(text(first.render.tree), '"text":"top" "text":"hello Ada" "text":"end" "text":"T"');
  // Only the content changed: the card's own props and the page's note are not touched.
  const updated = await update(first.render, { title: "T", who: "Grace" });
  assert.deepEqual(updated.patches.patches.map((p) => p.op), ["setText"]);
  const { render: full } = await render({ program: slotProgram, model: SLOT_MODEL, snapshot: { title: "T", who: "Grace" } });
  assert.deepEqual(updated.render.tree, full.tree);
  updated.render.release();
});

test("children for a composite with no slot are an assembly error, through the module", async () => {
  const bad = {
    root: "view",
    templates: [
      await template("view", "<page><card heading={title}><note>x</note></card></page>", SLOT_MODEL),
      await template("card", "<panel heading={heading} />", SLOT_MODEL),
    ],
  };
  const result = await render({ program: bad, model: SLOT_MODEL, snapshot: { title: "T", who: "Ada" } });
  assert.deepEqual(result.diagnostics.diagnostics.map((d) => d.code), ["assembly-composite-children"]);
});

// Composite events, through the module: a forward reaches the occurrence's command.
const EVENT_MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: { props: {}, events: {}, commands: {}, scope: {} },
    panel: { props: {}, events: {}, commands: {}, scope: {} },
    button: { props: {}, events: { tap: {} }, commands: {}, scope: {} },
    card: {
      props: { count: { type: { kind: "number" }, required: true } },
      events: { select: { payload: { kind: "number" } }, close: {} },
      commands: {},
      scope: { count: { kind: "number" } },
    },
    view: { props: {}, events: {}, commands: { pick: { parameters: [{ name: "n", type: { kind: "number" } }] } }, scope: { n: { kind: "number" } } },
  },
});
const eventProgram = {
  root: "view",
  templates: [
    await template("view", "<page><card count={n} on.select={pick($event)} /></page>", EVENT_MODEL),
    await template("card", "<panel><button on.tap={select(count)}>pick</button><button on.tap={close()}>close</button></panel>", EVENT_MODEL),
  ],
};

test("a composite's forwarded event reaches the command its occurrence binds, and an unbound one is not in the tree", async () => {
  const { render: first } = await render({ program: eventProgram, model: EVENT_MODEL, snapshot: { n: 7 } });
  const buttons = first.tree.root.children[0].children.filter((c) => c.type === "node");
  assert.equal(buttons.length, 2);
  assert.ok(buttons[0].events.tap, "`select` is bound");
  assert.deepEqual(buttons[1].events, {}, "`close` is not, so its button has no event");
  const result = await dispatch(first, buttons[0].events.tap);
  assert.deepEqual(result.intent.command, { component: "view", name: "pick" });
  assert.deepEqual(result.intent.arguments, [{ value: 7 }]);
  // And through an update: the same handler still reaches the same command, with the new value.
  const updated = await update(first, { n: 9 });
  const next = updated.render.tree.root.children[0].children.filter((c) => c.type === "node");
  assert.equal(next[0].events.tap, buttons[0].events.tap);
  assert.deepEqual((await dispatch(updated.render, next[0].events.tap)).intent.arguments, [{ value: 9 }]);
  updated.render.release();
});
