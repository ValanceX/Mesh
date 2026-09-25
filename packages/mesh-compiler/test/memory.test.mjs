// Memory stays bounded under repeated checks (outline v0.4 D6): after a
// warm-up pass, neither the instance's linear memory nor its count of
// live allocations grows, and each check releases everything it
// allocated. `MESH_MEMORY_CHECKS` sets the count (the Definition of
// Done's 100,000 by default).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { check, init } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { corpus, input, testModule } from "./common.mjs";

test("repeated checks don't grow memory", { timeout: 30 * 60 * 1000 }, async () => {
  await init(readFileSync(testModule));
  const inputs = corpus().map(input);
  for (const value of inputs) {
    await check(value);
  }
  const exports = instanceForTests();
  const live = () => exports.mesh_live_allocations();
  const bytes = () => exports.memory.buffer.byteLength;
  const baseline = { live: live(), bytes: bytes() };

  const total = Number(process.env.MESH_MEMORY_CHECKS ?? 100_000);
  for (let index = 0; index < total; index++) {
    const before = live();
    await check(inputs[index % inputs.length]);
    const after = live();
    if (after !== before) {
      assert.fail(`check ${index} left ${after - before} allocations live`);
    }
  }
  assert.equal(instanceForTests(), exports, "the instance was replaced");
  assert.deepEqual({ live: live(), bytes: bytes() }, baseline);
});
