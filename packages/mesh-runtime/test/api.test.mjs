// render() and dispatch() (D5, D9): results match their schemas, a
// render's tree is frozen, and dispatch takes only a Render render() made.
import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { dispatch, render, Render } from "../dist/index.js";
import {
  MODEL,
  programsDir,
  programTemplates,
  SNAPSHOT,
  validateDiagnostics,
  validateIntent,
  validateTree,
} from "./common.mjs";

const templates = await programTemplates(join(programsDir, "cards"));
const program = { root: "view", templates };

function handlers(node, event, out = []) {
  if (node.events[event]) out.push(node.events[event]);
  for (const child of node.children) if (child.type === "node") handlers(child, event, out);
  return out;
}

test("a render's tree matches render-v1, and is frozen", async () => {
  const result = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  assert.ok(result.render instanceof Render);
  assert.equal(result.diagnostics, undefined);
  const tree = result.render.tree;
  assert.ok(validateTree(tree), JSON.stringify(validateTree.errors));
  assert.ok(Object.isFrozen(tree) && Object.isFrozen(tree.root) && Object.isFrozen(tree.root.children));
  assert.ok(Object.isFrozen(result.render));
  assert.throws(() => {
    "use strict";
    tree.root.props.title = "changed";
  }, TypeError);
});

test("dispatch gives an intent that matches the schema", async () => {
  const { render: made } = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const [tap] = handlers(made.tree.root, "tap");
  const result = await dispatch(made, tap);
  assert.equal(result.diagnostics, undefined);
  assert.ok(validateIntent(result.intent), JSON.stringify(validateIntent.errors));
  assert.deepEqual(result.intent.command, { component: "card", name: "selectUser" });
  assert.ok(Object.isFrozen(result.intent));
});

test("problems with the inputs are diagnostics, and match the schema", async () => {
  const bad = await render({ program, model: MODEL, snapshot: { ...SNAPSHOT, count: "three" } });
  assert.equal(bad.render, undefined);
  assert.ok(validateDiagnostics(bad.diagnostics), JSON.stringify(validateDiagnostics.errors));
  assert.deepEqual(
    bad.diagnostics.diagnostics.map((d) => [d.code, d.location]),
    [["runtime-value-mismatch", { kind: "input", path: ["count"] }]],
  );
  const { render: made } = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const unknown = await dispatch(made, "hAAAAAAAAAAA.AAAAAAAAAAAAAAAAAAAAAA");
  assert.ok(validateDiagnostics(unknown.diagnostics));
  assert.deepEqual(
    unknown.diagnostics.diagnostics.map((d) => d.code),
    ["runtime-handler-other-program"],
  );
});

test("dispatch refuses a Render it didn't make, and a handler that isn't a string", async () => {
  await assert.rejects(dispatch({ tree: {} }, "h"), TypeError);
  await assert.rejects(dispatch(Object.create(Render.prototype), "h"), TypeError);
  assert.throws(() => new Render(Symbol("forged"), {}, "view", new Uint8Array(), "", new Uint8Array()), TypeError);
  const { render: made } = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  await assert.rejects(dispatch(made, 7), TypeError);
});

test("render refuses arguments of the wrong type, and a snapshot that isn't a plain object", async () => {
  await assert.rejects(render(null), TypeError);
  await assert.rejects(render({ program: { root: "view" }, model: MODEL, snapshot: {} }), TypeError);
  await assert.rejects(render({ program, model: 1, snapshot: {} }), TypeError);
  for (const snapshot of [undefined, null, [], 3, new Map()]) {
    await assert.rejects(render({ program, model: MODEL, snapshot }), TypeError, String(snapshot));
  }
  // The instance is fine afterwards.
  assert.ok((await render({ program, model: MODEL, snapshot: SNAPSHOT })).render);
});
