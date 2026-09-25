// Dispatch evaluates against the render's snapshot (D5): changing the
// host's objects after render changes nothing, and an earlier render
// gives the earlier values' intent after a later render.
import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { dispatch, render } from "../dist/index.js";
import { MODEL, programsDir, programTemplates, SNAPSHOT } from "./common.mjs";

const program = { root: "view", templates: await programTemplates(join(programsDir, "greeting")) };

/** The second `tap` handler: `setName(maybeName)`. */
function setName(made) {
  const taps = [];
  const walk = (node) => {
    if (node.events.tap) taps.push(node.events.tap);
    for (const child of node.children) if (child.type === "node") walk(child);
  };
  walk(made.tree.root);
  return taps[1];
}

test("changing the host's objects after render changes nothing", async () => {
  const snapshot = structuredClone({ ...SNAPSHOT, maybeName: "Countess" });
  const { render: made } = await render({ program, model: MODEL, snapshot });
  snapshot.maybeName = "Someone else";
  snapshot.user.name = 7; // would no longer even fit
  const { intent } = await dispatch(made, setName(made));
  assert.deepEqual(intent.arguments, [{ value: "Countess" }]);
});

test("an earlier render gives the earlier values' intent", async () => {
  const first = (await render({ program, model: MODEL, snapshot: { ...SNAPSHOT, maybeName: "Ada" } })).render;
  const second = (await render({ program, model: MODEL, snapshot: SNAPSHOT })).render;
  assert.deepEqual((await dispatch(second, setName(second))).intent.arguments, [{ absent: true }]);
  assert.deepEqual((await dispatch(first, setName(first))).intent.arguments, [{ value: "Ada" }]);
  // One program, so both trees have the same handler identifiers.
  assert.equal(setName(first), setName(second));
});

test("a payload not given, and one of undefined, are both absent", async () => {
  const { render: made } = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const click = made.tree.root.children.find((c) => c.type === "node" && c.events.click).events.click;
  const missing = await dispatch(made, click);
  const undef = await dispatch(made, click, undefined);
  assert.deepEqual(missing, undef);
  assert.deepEqual(missing.diagnostics.diagnostics.map((d) => d.code), ["runtime-missing-value"]);
  const given = await dispatch(made, click, { x: 1, y: -0 });
  assert.deepEqual(given.intent.arguments, [{ value: { x: 1, y: 0 } }]);
});
