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
  for (const patch of document.patches) {
    if (patch.op === "replace") return structuredClone(patch.tree);
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

test("a changed prop and a changed text are two small patches, and a changed structure is a replace", async () => {
  const { render: first } = await render({ program, model: MODEL, snapshot: snapshot("a", "x", false) });
  const small = await update(first, snapshot("b", "y", false));
  assert.deepEqual(small.patches.patches.map((p) => p.op), ["setProp", "setText"]);
  const structural = await update(first, snapshot("a", "x", true));
  assert.deepEqual(structural.patches.patches.map((p) => p.op), ["replace"]);
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
