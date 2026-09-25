// Memory stays bounded under repeated calls: after a warm-up, neither the
// instance's linear memory nor its count of live allocations grows, and
// each render and dispatch releases everything it allocated.
// `MESH_MEMORY_CALLS` sets the count (10,000 by default).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { dispatch, init, render } from "../dist/index.js";
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
