// The context channel: values a host gives beside the snapshot, read only by components whose manifest entry
// `uses` them. The package encodes them, keeps them with the render like the units, and hands them back to the
// module; it judges nothing (the Rust runtime's tests and the conformance vectors are the judge). These pin what
// is the package's own: the argument, and that a render keeps the context it was made with, however it was made.
import assert from "node:assert/strict";
import { test } from "node:test";
import { MeshUsageError, dispatch, openScreen, render, update, updateChanges } from "../dist/index.js";
import { template } from "./common.mjs";

const MODEL = JSON.stringify({
  version: 1,
  types: {},
  context: { theme: { kind: "string" } },
  components: {
    box: { props: { tone: { type: { kind: "string" }, required: false } }, events: {}, commands: {}, scope: {} },
    button: { props: {}, events: { click: {} }, commands: {}, scope: {} },
    badge: { props: {}, events: {}, commands: { pick: { parameters: [{ name: "theme", type: { kind: "string" } }] } }, scope: {}, uses: ["theme"] },
    page: { props: {}, events: {}, commands: {}, scope: { title: { kind: "string" } } },
  },
});

async function program() {
  return {
    root: "page",
    templates: [
      await template("page", "<box tone={title}><badge /></box>", MODEL),
      await template("badge", "<button on.click={pick(theme)} />", MODEL),
    ],
  };
}

const handlerOf = (made) => made.tree.root.children[0].events.click;

test("a render keeps its context: dispatch, an update that gives none, and one derived again all use it", async () => {
  const { render: first } = await render({ program: await program(), model: MODEL, snapshot: { title: "a" }, context: { theme: "dark" } });
  assert.deepEqual((await dispatch(first, handlerOf(first))).intent.arguments, [{ value: "dark", name: "theme" }]);
  // Changed in the module, by changes: the package learns the new value from the module's result.
  const changed = await updateChanges(first, { base: first.version, changes: [{ op: "set", path: [{ context: "theme" }], value: "light" }] });
  assert.ok(changed.render, JSON.stringify(changed.diagnostics));
  const kept = await update(changed.render, { title: "b" });
  kept.render.release();
  // Released, so the module derives it again from the inputs the package kept: the context the module reported.
  assert.deepEqual((await dispatch(kept.render, handlerOf(kept.render))).intent.arguments, [{ value: "light", name: "theme" }]);
  const again = await update(kept.render, { title: "c" });
  assert.ok(again.render, JSON.stringify(again.diagnostics));
  assert.deepEqual((await dispatch(again.render, handlerOf(again.render))).intent.arguments, [{ value: "light", name: "theme" }]);
  // Given anew, it is the new one.
  const given = await update(again.render, { title: "c" }, undefined, { theme: "blue" });
  assert.deepEqual((await dispatch(given.render, handlerOf(given.render))).intent.arguments, [{ value: "blue", name: "theme" }]);
  for (const each of [changed, kept, again, given]) each.render.release();
});

test("context is an object of values; the runtime judges them", async () => {
  await assert.rejects(render({ program: await program(), model: MODEL, snapshot: { title: "a" }, context: "dark" }), MeshUsageError);
  const missing = await render({ program: await program(), model: MODEL, snapshot: { title: "a" } });
  assert.equal(missing.diagnostics.diagnostics[0].code, "runtime-missing-value");
  assert.deepEqual(missing.diagnostics.diagnostics[0].location, { kind: "input", path: ["$context", "theme"] });
});

test("a screen shows with context: the first tree reads it, and a new context between shows gives patches", async () => {
  // The badge shows the theme, so a theme change is visible in the tree.
  const shown = {
    root: "page",
    templates: [await template("page", "<box tone={title}><badge /></box>", MODEL), await template("badge", "<box tone={theme} />", MODEL)],
  };
  const screen = openScreen({ program: shown, model: MODEL });
  const first = await screen.show({ title: "a" }, { theme: "dark" });
  assert.ok(first.tree, JSON.stringify(first.diagnostics));
  assert.ok(JSON.stringify(first.tree).includes('"dark"'));
  // The snapshot is the same; only the context changed, so the patches carry the new theme.
  const changed = await screen.show({ title: "a" }, { theme: "light" });
  assert.ok(changed.patches, JSON.stringify(changed.diagnostics));
  assert.ok(JSON.stringify(changed.patches).includes('"light"'), JSON.stringify(changed.patches));
  // Shown without context, the screen keeps the values last shown: only the title changes.
  const kept = await screen.show({ title: "b" });
  assert.ok(kept.patches, JSON.stringify(kept.diagnostics));
  assert.ok(!JSON.stringify(kept.patches).includes('"light"') && !JSON.stringify(kept.patches).includes('"dark"'), JSON.stringify(kept.patches));
  screen.close();
  // A required context type with no value is a diagnostic, as render gives.
  assert.equal((await screen.show({ title: "a" })).diagnostics.diagnostics[0].code, "runtime-missing-value");
});
