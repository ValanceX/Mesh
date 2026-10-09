// The failure contract at the WebAssembly boundary: a trap becomes
// MeshInternalError, the failed instance is never called again, and the
// next call is answered by a fresh one. A refused value (a TypeError) is
// not a failure: the instance stays.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { dispatch, init, MeshInternalError, render } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { MODEL, SNAPSHOT, template, testModule } from "./common.mjs";

const program = {
  root: "view",
  templates: [await template("view", "<probe num={count} on.tap={save()} />")],
};
const input = { program, model: MODEL, snapshot: SNAPSHOT };

function neverCalledAgain(failed) {
  let calls = 0;
  for (const [name, value] of Object.entries(failed)) {
    if (typeof value === "function") {
      failed[name] = () => {
        calls += 1;
        throw new Error(`the failed instance was called: ${name}`);
      };
    }
  }
  return () => calls;
}

test("a panic in render is a MeshInternalError, and the instance is never reused", async () => {
  await init(readFileSync(testModule));
  const expected = await render(input);
  const failed = instanceForTests();
  failed.mesh_render = () => failed.mesh_test_panic();
  const error = await render(input).then(
    () => assert.fail("render should have failed"),
    (error) => error,
  );
  assert.ok(error instanceof MeshInternalError, String(error));
  assert.ok(error.cause instanceof WebAssembly.RuntimeError, String(error.cause));
  const calls = neverCalledAgain(failed);
  assert.deepEqual((await render(input)).render.tree, expected.render.tree);
  assert.equal(calls(), 0);
  assert.notEqual(instanceForTests(), failed);
});

test("a panic in dispatch is a MeshInternalError, and the next dispatch works", async () => {
  await init(readFileSync(testModule));
  const { render: made } = await render(input);
  const tap = made.tree.root.events.tap;
  const expected = await dispatch(made, tap);
  const failed = instanceForTests();
  failed.mesh_dispatch = () => failed.mesh_test_panic();
  await assert.rejects(dispatch(made, tap), MeshInternalError);
  const calls = neverCalledAgain(failed);
  assert.deepEqual(await dispatch(made, tap), expected, "a render outlives the instance that made it");
  assert.equal(calls(), 0);
});

test("a result that isn't a document is a MeshInternalError", async () => {
  await init(readFileSync(testModule));
  const exports = instanceForTests();
  exports.mesh_result_len = () => 0;
  await assert.rejects(render(input), MeshInternalError);
  assert.ok((await render(input)).render);
});

test("bytes the module refuses are a MeshInternalError", async () => {
  await init(readFileSync(testModule));
  const exports = instanceForTests();
  const real = exports.mesh_render;
  exports.mesh_render = (...args) => real(...args.slice(0, 2), 0, 0, ...args.slice(4));
  await assert.rejects(render(input), MeshInternalError);
  assert.ok((await render(input)).render);
});

test("a refused value is a TypeError, and the instance stays", async () => {
  await init(readFileSync(testModule));
  const before = instanceForTests();
  await assert.rejects(render({ ...input, snapshot: [] }), TypeError);
  let deep = 1;
  for (let level = 0; level < 200; level++) deep = [deep];
  await assert.rejects(render({ ...input, snapshot: { ...SNAPSHOT, anything: deep } }), /nests deeper than 128/);
  assert.equal(instanceForTests(), before);
});
