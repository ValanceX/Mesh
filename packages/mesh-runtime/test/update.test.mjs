// update(): the new render and the patches from the previous tree to its
// tree. The law (applying the patches gives exactly a full render's tree)
// is the same as the Rust runtime's, and a refused update leaves the
// previous render valid.
import assert from "node:assert/strict";
import { test } from "node:test";
import { dispatch, render, update } from "../dist/index.js";
import { template, validatePatches } from "./common.mjs";

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
  assert.equal(retained(), 0, "render() keeps nothing in the module");
  for (const [index, next] of states.slice(1).entries()) {
    const updated = await update(current, next);
    // The new render is kept, and so is the previous one if an update made it, until released.
    assert.equal(retained(), index === 0 ? 1 : 2);
    const { render: full } = await render({ program, model: MODEL, snapshot: next });
    assert.deepEqual(updated.render.tree, full.tree);
    assert.deepEqual(apply(current.tree, updated.patches), full.tree);
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
  failed.mesh_render = () => failed.mesh_test_panic();
  await assert.rejects(render({ program, model: MODEL, snapshot: snapshot("a", "x", false) }));
  assert.notEqual(instanceForTests(), failed);
  const two = await update(one.render, snapshot("c", "x", false));
  assert.equal(two.diagnostics, undefined);
  assert.equal(two.render.tree.root.props.title, "c");
  two.render.release();
});
