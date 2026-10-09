// Memory stays bounded under repeated calls: after a warm-up, neither the
// instance's linear memory nor its count of live allocations grows, and
// each render and dispatch releases everything it allocated (a render is released by the caller, as it must be).
// `MESH_MEMORY_CALLS` sets the count (10,000 by default).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { dispatch, init, render, update } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { MODEL, programsDir, programTemplates, SNAPSHOT, testModule } from "./common.mjs";

test("repeated renders and dispatches don't grow memory", { timeout: 30 * 60 * 1000 }, async () => {
  await init(readFileSync(testModule));
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const inputs = [
    { program, model: MODEL, snapshot: SNAPSHOT },
    { program, model: MODEL, snapshot: { ...SNAPSHOT, count: "bad" } },
  ];
  const once = async (index) => {
    const result = await render(inputs[index % 2]);
    if (result.render) {
      const node = result.render.tree.root.children.find((c) => c.type === "node");
      const [handler] = Object.values(node.events).concat(["hX"]);
      await dispatch(result.render, handler);
      result.render.release();
      await new Promise((resolve) => setTimeout(resolve, 0)); // the release is queued behind the calls
    }
  };
  for (let index = 0; index < 20; index++) await once(index);
  const exports = instanceForTests();
  const live = () => exports.mesh_live_allocations();
  const bytes = () => exports.memory.buffer.byteLength;
  const baseline = { live: live(), bytes: bytes() };
  const total = Number(process.env.MESH_MEMORY_CALLS ?? 10_000);
  for (let index = 0; index < total; index++) {
    const before = live();
    await once(index);
    if (live() !== before) {
      assert.fail(`call ${index} left ${live() - before} allocations live`);
    }
  }
  assert.equal(live(), baseline.live);
  assert.equal(bytes(), baseline.bytes, "linear memory didn't grow");
});

test("a long chain of updates, each releasing the render before it, doesn't grow memory", { timeout: 30 * 60 * 1000 }, async () => {
  await init(readFileSync(testModule));
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  let { render: current } = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const exports = instanceForTests();
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0)); // releases queue behind calls
  const step = async (index) => {
    const next = { ...SNAPSHOT, count: index % 7 };
    const updated = await update(current, next);
    assert.equal(updated.diagnostics, undefined);
    current.release();
    current = updated.render;
    await settle();
  };
  for (let index = 0; index < 20; index++) await step(index);
  const baseline = { live: exports.mesh_live_allocations(), bytes: exports.memory.buffer.byteLength };
  assert.equal(exports.mesh_retained_renders(), 1, "only the current render is kept");
  const total = Number(process.env.MESH_MEMORY_CALLS ?? 10_000);
  for (let index = 0; index < total; index++) {
    await step(index);
    assert.equal(exports.mesh_retained_renders(), 1, `after update ${index}`);
  }
  assert.equal(exports.mesh_live_allocations(), baseline.live);
  assert.equal(exports.memory.buffer.byteLength, baseline.bytes, "linear memory didn't grow");
  current.release();
  await settle();
  assert.equal(exports.mesh_retained_renders(), 0);
});

test("renders a host never releases don't accumulate in the module (render() keeps nothing)", { timeout: 30 * 60 * 1000 }, async () => {
  await init(readFileSync(testModule));
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const exports = instanceForTests();
  const total = Number(process.env.MESH_MEMORY_CALLS ?? 10_000);
  for (let index = 0; index < 20; index++) await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const baseline = { live: exports.mesh_live_allocations(), bytes: exports.memory.buffer.byteLength };
  for (let index = 0; index < total; index++) await render({ program, model: MODEL, snapshot: SNAPSHOT }); // a 0.9.0 host: never releases
  assert.equal(exports.mesh_retained_renders(), 0);
  assert.equal(exports.mesh_live_allocations(), baseline.live);
  assert.equal(exports.memory.buffer.byteLength, baseline.bytes, "linear memory didn't grow");
});
