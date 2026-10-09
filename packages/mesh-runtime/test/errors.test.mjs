// The package's thrown errors have stable codes (API principles, 9.1), and a
// render can be released with `using` (7).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { dispatch, init, MeshUsageError, render, update, updateChanges } from "../dist/index.js";
import { testModule } from "./common.mjs";
import { LIST_MODEL, listProgram } from "./common/list-program.mjs";

await init(readFileSync(testModule));
const snapshot = { title: "t", count: 0, flag: true, items: [] };

test("a call with the wrong kind of argument is a MeshUsageError with the code invalid-argument, and still a TypeError", async () => {
  await assert.rejects(render({ program: { root: 1, templates: [] }, model: LIST_MODEL, snapshot }), (error) => {
    assert.ok(error instanceof MeshUsageError && error instanceof TypeError);
    assert.equal(error.code, "invalid-argument");
    return true;
  });
});

test("a Render this package did not make is refused with the code not-a-render", async () => {
  await assert.rejects(update({}, snapshot), (error) => error instanceof MeshUsageError && error.code === "not-a-render");
  await assert.rejects(dispatch({}, "h"), (error) => error instanceof MeshUsageError && error.code === "not-a-render");
});

test("a released render that has no snapshot of its own is refused with the code render-gone", async () => {
  const { render: kept } = await render({ program: listProgram, model: LIST_MODEL, snapshot, keep: true });
  const made = await updateChanges(kept, { base: kept.version, changes: [{ op: "set", path: ["count"], value: 1 }] });
  made.render.release();
  await assert.rejects(update(made.render, snapshot), (error) => error instanceof MeshUsageError && error.code === "render-gone");
  await assert.rejects(updateChanges(made.render, { base: 0, changes: [] }), (error) => error.code === "render-gone");
  kept.release();
});

test("a render has [Symbol.dispose], which does what release() does (the `using` statement calls it)", async () => {
  const kept = (await render({ program: listProgram, model: LIST_MODEL, snapshot, keep: true })).render;
  assert.equal(typeof kept.version, "number");
  assert.equal(typeof kept[Symbol.dispose], "function");
  kept[Symbol.dispose]();
  assert.equal(kept.version, undefined, "released");
  kept[Symbol.dispose](); // safe twice, as release() is
});
