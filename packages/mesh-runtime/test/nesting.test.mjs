// Nesting is limited (decision U12): a render nested past 512 levels is a
// diagnostic, never a MeshInternalError, at the depth that used to crash the
// module (about 690 nested composites), and a template at MPRX's documented
// nesting of 128 elements reads and renders.
import assert from "node:assert/strict";
import { test } from "node:test";
import { render } from "../dist/index.js";
import { template } from "./common.mjs";

const component = '{ "props": {}, "events": {}, "commands": {}, "scope": {} }';
const chain = async (depth, perLevel) => {
  const names = ["box", ...Array.from({ length: depth + 1 }, (_, i) => `c${i}`)];
  const model = `{ "version": 1, "types": {}, "components": { ${names
    .map((name) => `"${name}": ${component}`)
    .join(", ")} } }`;
  const templates = [];
  for (let i = 0; i <= depth; i++) {
    const inner = i < depth ? `<c${i + 1} />` : "end";
    const source = "<box>".repeat(perLevel) + inner + "</box>".repeat(perLevel);
    templates.push(await template(`c${i}`, source, model));
  }
  return { program: { root: "c0", templates }, model, snapshot: {} };
};

test("nesting within the cap renders", async () => {
  assert.ok((await render(await chain(100, 1))).render);
});

for (const [depth, perLevel] of [
  [600, 1],
  [1000, 1],
  [100, 10],
]) {
  test(`${depth} composites of ${perLevel} element(s) are a diagnostic`, async () => {
    const result = await render(await chain(depth, perLevel));
    assert.equal(result.render, undefined);
    assert.deepEqual(
      result.diagnostics.diagnostics.map((d) => d.code),
      ["runtime-nesting-too-deep"],
    );
  });
}

test("128 nested elements in one template read and render", async () => {
  const model = `{ "version": 1, "types": {}, "components": { "box": ${component}, "c0": ${component} } }`;
  const source = "<box>".repeat(128) + "end" + "</box>".repeat(128);
  const result = await render({
    program: { root: "c0", templates: [await template("c0", source, model)] },
    model,
    snapshot: {},
  });
  assert.ok(result.render, JSON.stringify(result.diagnostics));
});
