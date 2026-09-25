// The slice (outline, "The slice"), in Node: `examples/slice/`, compiled
// with compile(), checked with checkProgram(), rendered and dispatched
// through this package, and compared with the committed files the
// native test (`crates/mesh-runtime/tests/slice.rs`) blessed.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { checkProgram, compile } from "../../mesh-compiler/dist/index.js";
import { dispatch, render } from "../dist/index.js";
import { root } from "./common.mjs";
import { canonical } from "./common/json.mjs";
import { compare, print } from "./common/renderer.mjs";

const slice = join(root, "examples", "slice");
const read = (name) => readFileSync(join(slice, name), "utf8");
const model = read("components.json");

async function templates() {
  const out = [];
  for (const component of ["users", "user-card"]) {
    const result = await compile({
      source: read(`${component}.mprx`),
      path: `${component}.mprx`,
      model: { manifest: model, path: "components.json", component },
    });
    assert.deepEqual(result.diagnostics.diagnostics, [], component);
    out.push(JSON.stringify(result.template));
  }
  return out;
}

function nodes(node, out = []) {
  out.push(node);
  for (const child of node.children) if (child.type === "node") nodes(child, out);
  return out;
}

test("the slice renders, dispatches and re-renders as committed", async () => {
  const program = { root: "users", templates: await templates() };
  assert.deepEqual(await checkProgram({ model, ...program }), { version: 1, diagnostics: [] });

  const first = await render({ program, model, snapshot: JSON.parse(read("snapshots/first.json")) });
  const tree = first.render.tree;
  assert.ok(nodes(tree.root).every((node) => node.component !== "user-card"), "primitives only");
  assert.equal(`${canonical(tree)}\n`, read("expected/first.tree.json"));
  assert.equal(print(tree), read("expected/first.html"));

  const click = nodes(tree.root).find((node) => node.component === "avatar").events.click;
  const { intent } = await dispatch(first.render, click, { x: 12, y: 34 });
  assert.equal(`${canonical(intent)}\n`, read("expected/select-first.intent.json"));

  const second = await render({ program, model, snapshot: JSON.parse(read("snapshots/second.json")) });
  assert.equal(compare(tree, second.render.tree), read("expected/first-to-second.changes"));
});
