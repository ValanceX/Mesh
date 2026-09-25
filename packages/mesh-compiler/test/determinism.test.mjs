// Identical inputs give identical documents (outline v0.4 I9): twice on
// one instance, and again on a fresh instance after a failure.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { check, init, MeshInternalError } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { corpus, input, testModule } from "./common.mjs";

test("the same inputs give the same document, before and after a failure", async () => {
  await init(readFileSync(testModule));
  const runs = corpus();
  const first = [];
  for (const run of runs) {
    first.push(await check(input(run)));
  }
  for (const [index, run] of runs.entries()) {
    assert.deepEqual(await check(input(run)), first[index], run.file);
  }

  // Make the next check trap, so the wrapper replaces its instance.
  const exports = instanceForTests();
  exports.mesh_check = () => exports.mesh_test_panic();
  await assert.rejects(check(input(runs[0])), MeshInternalError);
  for (const [index, run] of runs.entries()) {
    assert.deepEqual(await check(input(run)), first[index], run.file);
  }
  assert.notEqual(instanceForTests(), exports);
});
