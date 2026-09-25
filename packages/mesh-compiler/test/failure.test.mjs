// The failure contract at the WASM boundary (outline v0.4 D6): a trap
// becomes MeshInternalError, the failed instance is never called again,
// and the next check is answered by a fresh one.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { check, init, MeshInternalError } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { cli, read, testModule } from "./common.mjs";

test("a panic is a MeshInternalError, and the instance is never reused", async () => {
  await init(readFileSync(testModule));
  const source = read("fixtures/fail/mismatched-closing-tag.mprx");
  const value = { source, path: "fixtures/fail/mismatched-closing-tag.mprx" };
  const expected = cli(value.path);
  assert.deepEqual(await check(value), expected);

  const failed = instanceForTests();
  failed.mesh_check = () => failed.mesh_test_panic();
  const error = await check(value).then(
    () => assert.fail("the check should have failed"),
    (error) => error,
  );
  assert.ok(error instanceof MeshInternalError, String(error));
  assert.ok(error.cause instanceof WebAssembly.RuntimeError, String(error.cause));

  // Count every call into the failed instance from here on.
  let calls = 0;
  for (const [name, value] of Object.entries(failed)) {
    if (typeof value === "function") {
      failed[name] = () => {
        calls += 1;
        throw new Error(`the failed instance was called: ${name}`);
      };
    }
  }
  assert.deepEqual(await check(value), expected);
  assert.equal(calls, 0);
  assert.notEqual(instanceForTests(), failed);
});

test("a result that isn't a document is a MeshInternalError", async () => {
  await init(readFileSync(testModule));
  const exports = instanceForTests();
  exports.mesh_result_len = () => 0;
  await assert.rejects(check({ source: "<a />", path: "a.mprx" }), MeshInternalError);
  assert.deepEqual(await check({ source: "<a />", path: "a.mprx" }), { version: 1, diagnostics: [] });
});
