// Changes: `diff` (the host-side change from one snapshot to the next) and
// `updateChanges` (the runtime's update by edits, not a whole new snapshot).
// The law is the update's: the render updateChanges makes is the render of the
// snapshot the edits make, and applying `diff(a, b)` to `a` gives `b`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { diff, dispatch, init, render, update, updateChanges } from "../dist/index.js";
import { instanceForTests } from "../dist/engine.js";
import { testModule, validatePatches } from "./common.mjs";
import { applyChanges, clean } from "./common/apply-changes.mjs";
import { generator, LIST_MODEL, listProgram, mutate } from "./common/list-program.mjs";

const START = { title: "t", count: 0, flag: true, items: [{ id: 0, label: "a", done: false }, { id: 1, label: "b", done: true }] };
const program = listProgram;
const model = LIST_MODEL;
const first = (snapshot) => render({ program, model, snapshot, keep: true });
const rowHandler = (tree) => tree.root.children.find((c) => c.type === "node" && c.component === "row").events.tap;

// --- diff --------------------------------------------------------------------

test("diff: applying it to the first snapshot gives the second, over random snapshots", () => {
  const below = generator(31);
  let pairs = 0;
  for (let chain = 0; chain < 80; chain++) {
    let state = { title: "t", count: 0, flag: false, items: [] };
    for (let step = 0; step < 25; step++) {
      const next = mutate(state, below);
      assert.deepStrictEqual(clean(applyChanges(state, diff(state, next))), clean(next), JSON.stringify([state, next]));
      state = next;
      pairs++;
    }
  }
  // And between unrelated snapshots, not only neighbours.
  const states = [];
  let walk = START;
  for (let i = 0; i < 60; i++) states.push((walk = mutate(walk, below)));
  for (const a of states) for (const b of states.slice(0, 12)) {
    assert.deepStrictEqual(clean(applyChanges(a, diff(a, b))), clean(b));
    pairs++;
  }
  assert.ok(pairs > 2000);
});

test("diff: nothing changed is no changes, and one changed field is one edit", () => {
  assert.deepEqual(diff(START, structuredClone(START)), []);
  const next = structuredClone(START);
  next.items[1].label = "B";
  assert.deepEqual(diff(START, next), [{ op: "set", path: ["items", 1, "label"], value: "B" }]);
});

test("diff: a list gaining, losing or replacing elements uses its common start and end", () => {
  const items = (...ids) => ({ items: ids.map((id) => ({ id })) });
  assert.deepEqual(diff(items(1, 2, 3), items(1, 2, 3, 4)), [{ op: "insert", path: ["items", 3], value: { id: 4 } }]);
  assert.deepEqual(diff(items(1, 2, 3), items(1, 3)), [{ op: "remove", path: ["items", 1] }]);
  assert.deepEqual(diff(items(1, 3), items(1, 2, 3)), [{ op: "insert", path: ["items", 1], value: { id: 2 } }]);
  assert.deepEqual(diff(items(1, 2, 3), items(1, 9, 3)), [{ op: "set", path: ["items", 1, "id"], value: 9 }]);
  assert.deepEqual(diff(items(1, 2, 3, 4, 5), items(1, 5)), [
    { op: "remove", path: ["items", 1] },
    { op: "remove", path: ["items", 1] },
    { op: "remove", path: ["items", 1] },
  ]);
});

test("diff: a field gone is a remove, a field new is a set, and undefined is absent", () => {
  assert.deepEqual(diff({ a: 1, b: 2 }, { a: 1 }), [{ op: "remove", path: ["b"] }]);
  assert.deepEqual(diff({ a: 1 }, { a: 1, b: 2 }), [{ op: "set", path: ["b"], value: 2 }]);
  assert.deepEqual(diff({ a: 1, b: undefined }, { a: 1 }), []);
  assert.deepEqual(diff({ a: 1 }, { a: 1, b: undefined }), []);
  assert.deepEqual(diff({ a: { x: 1 } }, { a: { x: undefined } }), [{ op: "remove", path: ["a", "x"] }]);
});

test("diff: -0 and 0 are different values, as the boundary has them", () => {
  assert.deepEqual(diff({ n: 0 }, { n: -0 }), [{ op: "set", path: ["n"], value: -0 }]);
  assert.deepEqual(diff({ n: -0 }, { n: 0 }), [{ op: "set", path: ["n"], value: 0 }]);
  assert.deepEqual(diff({ n: -0 }, { n: -0 }), []);
});

test("diff: what isn't plain data is given whole, for the runtime to judge", () => {
  const date = new Date(0);
  assert.deepEqual(diff({ d: 1 }, { d: date }), [{ op: "set", path: ["d"], value: date }]);
  const map = new Map();
  assert.deepEqual(diff({ m: { a: 1 } }, { m: map }), [{ op: "set", path: ["m"], value: map }]);
  // A hole or undefined inside a list: the list is given whole, to be refused there.
  const holey = [1, undefined, 3];
  assert.deepEqual(diff({ l: [1, 2, 3] }, { l: holey }), [{ op: "set", path: ["l"], value: holey }]);
  // Nesting past the runtime's limit is given whole, not walked into without end.
  let deep = { end: 1 };
  let other = { end: 2 };
  for (let i = 0; i < 300; i++) {
    deep = { inner: deep };
    other = { inner: other };
  }
  const edits = diff({ v: deep }, { v: other });
  assert.equal(edits.length, 1);
  assert.equal(edits[0].op, "set");
});

// --- updateChanges -----------------------------------------------------------

test("a render kept has a version, and one not kept has none", async () => {
  const kept = await first(START);
  assert.equal(typeof kept.render.version, "number");
  const plain = await render({ program, model, snapshot: START });
  assert.equal(plain.render.version, undefined);
  const other = await first(START);
  assert.notEqual(other.render.version, kept.render.version);
  kept.render.release();
  assert.equal(kept.render.version, undefined, "released: not in the module");
  other.render.release();
});

test("over random chains of changes, the render is the render of the snapshot, verified every time", async () => {
  const below = generator(77);
  let checked = 0;
  for (let chain = 0; chain < 40; chain++) {
    let state = { title: "t", count: 0, flag: false, items: [] };
    let { render: current } = await first(state);
    for (let step = 0; step < 20; step++) {
      const next = mutate(state, below);
      const changes = { base: current.version, changes: diff(state, next) };
      // Verify mode: the whole snapshot the host believes it has is checked against the edits.
      const updated = await updateChanges(current, changes, { verify: next });
      assert.equal(updated.diagnostics, undefined, JSON.stringify([updated.diagnostics, changes]));
      assert.ok(validatePatches(updated.patches), JSON.stringify(validatePatches.errors));
      const { render: full } = await render({ program, model, snapshot: next });
      assert.deepEqual(updated.render.tree, full.tree);
      // The same changes by the whole-snapshot form give the same tree.
      const whole = await update(current, next);
      assert.deepEqual(updated.render.tree, whole.render.tree);
      whole.render.release();
      current.release();
      current = updated.render;
      state = next;
      checked++;
    }
    current.release();
  }
  assert.equal(checked, 800);
});

test("the new tree shares every part the changes didn't reach", async () => {
  const items = Array.from({ length: 60 }, (_, id) => ({ id, label: `l${id}`, done: false }));
  const state = { title: "t", count: 0, flag: true, items };
  const { render: kept } = await first(state);
  const next = structuredClone(state);
  next.items[30].label = "changed";
  const updated = await updateChanges(kept, { base: kept.version, changes: diff(state, next) });
  const before = kept.tree.root.children;
  const after = updated.render.tree.root.children;
  assert.equal(before.length, after.length);
  assert.equal(after.filter((child, index) => child !== before[index]).length, 1);
  kept.release();
  updated.render.release();
});

test("changes are applied only to the render they name", async () => {
  const { render: a } = await first(START);
  const { render: b } = await first(START);
  const edit = diff(START, { ...START, count: 5 });
  const wrong = await updateChanges(a, { base: b.version, changes: edit });
  assert.equal(wrong.render, undefined);
  assert.deepEqual(wrong.diagnostics.diagnostics.map((d) => d.code), ["runtime-changes-base-mismatch"]);
  // `a` is untouched and takes its own changes; and the render those made is not a base for them again.
  const right = await updateChanges(a, { base: a.version, changes: edit });
  assert.equal(right.diagnostics, undefined);
  const again = await updateChanges(right.render, { base: a.version, changes: edit });
  assert.deepEqual(again.diagnostics.diagnostics.map((d) => d.code), ["runtime-changes-base-mismatch"]);
  for (const r of [a, b, right.render]) r.release();
});

test("an invalid change is a diagnostic at its path, and the render is untouched", async () => {
  const { render: kept } = await first(START);
  const bad = [
    [{ op: "set", path: ["nope"], value: 1 }, "runtime-invalid-change"],
    [{ op: "set", path: ["items", 9, "label"], value: "x" }, "runtime-invalid-change"],
    [{ op: "remove", path: ["items", 0, "label"] }, "runtime-invalid-change"],
    [{ op: "set", path: ["count"], value: "three" }, "runtime-value-mismatch"],
    [{ op: "set", path: ["items", 0, "done"], value: 1 }, "runtime-value-mismatch"],
    [{ op: "set", path: ["items", 0], value: { id: 1, label: "x" } }, "runtime-missing-value"],
  ];
  for (const [change, code] of bad) {
    const result = await updateChanges(kept, { base: kept.version, changes: [change] });
    assert.equal(result.render, undefined);
    assert.equal(result.diagnostics.diagnostics[0].code, code, JSON.stringify(change));
  }
  // A malformed document is a diagnostic too, with where.
  const malformed = await updateChanges(kept, { base: kept.version, changes: [{ op: "zap", path: ["count"] }] });
  assert.equal(malformed.diagnostics.diagnostics[0].code, "runtime-invalid-change");
  assert.deepEqual(malformed.diagnostics.diagnostics[0].location.path, ["changes", 0, "op"]);
  // Still good.
  const ok = await updateChanges(kept, { base: kept.version, changes: [{ op: "set", path: ["count"], value: 2 }] });
  assert.equal(ok.diagnostics, undefined);
  kept.release();
  ok.render.release();
});

test("verify mode catches changes that make a snapshot other than the one the host believes", async () => {
  const { render: kept } = await first(START);
  const believed = { ...START, title: "right" };
  const wrong = { base: kept.version, changes: [{ op: "set", path: ["title"], value: "wrong" }] };
  const caught = await updateChanges(kept, wrong, { verify: believed });
  assert.deepEqual(caught.diagnostics.diagnostics.map((d) => d.code), ["runtime-changes-disagree"]);
  assert.deepEqual(caught.diagnostics.diagnostics[0].location.path, ["title"]);
  // Without verify mode the wrong edit is applied: that is what verify mode is for.
  const applied = await updateChanges(kept, wrong);
  assert.equal(applied.diagnostics, undefined);
  kept.release();
  applied.render.release();
});

test("a render not in the module can't take changes, and says why", async () => {
  const plain = await render({ program, model, snapshot: START });
  await assert.rejects(updateChanges(plain.render, { base: 1, changes: [] }), TypeError);
  const { render: kept } = await first(START);
  kept.release();
  await assert.rejects(updateChanges(kept, { base: 1, changes: [] }), /needs this render to be in the module/);
});

test("a render made from changes dispatches in the module, and can be updated by either form", async () => {
  const { render: kept } = await first(START);
  const next = { ...START, count: 4, items: [{ id: 7, label: "z", done: false }, ...START.items] };
  const made = await updateChanges(kept, { base: kept.version, changes: diff(START, next) });
  const handler = rowHandler(made.render.tree);
  const result = await dispatch(made.render, handler);
  assert.deepEqual(result.intent.command, { component: "view", name: "pick" });
  assert.deepEqual(result.intent.arguments, [{ value: 7 }], "the id the changes gave");
  // The whole-snapshot form from it, and the changes form from that.
  const whole = await update(made.render, { ...next, title: "W" });
  assert.equal(whole.render.tree.root.children[0].children[0].text, "W has 4");
  const again = await updateChanges(whole.render, { base: whole.render.version, changes: [{ op: "set", path: ["title"], value: "C" }] });
  assert.equal(again.render.tree.root.children[0].children[0].text, "C has 4");
  for (const r of [kept, made.render, whole.render, again.render]) r.release();
});

test("once released, a render made from changes can't be used: it has no snapshot outside the module", async () => {
  const { render: kept } = await first(START);
  const made = await updateChanges(kept, { base: kept.version, changes: [{ op: "set", path: ["count"], value: 2 }] });
  const handler = rowHandler(made.render.tree);
  made.render.release();
  await assert.rejects(dispatch(made.render, handler), /copy in the module, which is gone/);
  await assert.rejects(update(made.render, START), /copy in the module, which is gone/);
  // But a render with its own snapshot still works after release, by deriving itself again.
  const { render: own } = await first(START);
  own.release();
  assert.equal((await dispatch(own, rowHandler(own.tree))).intent.command.name, "pick");
  kept.release();
});

test("when the module is replaced, a render with a snapshot of its own survives and one made from changes does not", async () => {
  const { render: kept } = await first(START);
  const made = await updateChanges(kept, { base: kept.version, changes: [{ op: "set", path: ["count"], value: 2 }] });
  const failed = instanceForTests();
  failed.mesh_render = () => failed.mesh_test_panic();
  await assert.rejects(render({ program, model, snapshot: START }));
  assert.notEqual(instanceForTests(), failed);
  await assert.rejects(updateChanges(kept, { base: kept.version, changes: [] }), TypeError);
  await assert.rejects(dispatch(made.render, rowHandler(made.render.tree)), /gone/);
  // `kept` has its own snapshot: whole-snapshot update derives it again.
  const again = await update(kept, { ...START, count: 9 });
  assert.equal(again.diagnostics, undefined);
  again.render.release();
});

test("a long chain of changes, releasing as it goes, doesn't grow memory", { timeout: 30 * 60 * 1000 }, async () => {
  await init(readFileSync(testModule));
  const state = { title: "t", count: 0, flag: true, items: Array.from({ length: 20 }, (_, id) => ({ id, label: `l${id}`, done: false })) };
  let { render: current } = await first(state);
  const exports = instanceForTests();
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
  const step = async (index) => {
    const updated = await updateChanges(current, { base: current.version, changes: [{ op: "set", path: ["count"], value: index % 9 }] });
    assert.equal(updated.diagnostics, undefined);
    current.release();
    current = updated.render;
    await settle();
  };
  for (let index = 0; index < 20; index++) await step(index);
  const baseline = { live: exports.mesh_live_allocations(), bytes: exports.memory.buffer.byteLength };
  const total = Number(process.env.MESH_MEMORY_CALLS ?? 10_000);
  for (let index = 0; index < total; index++) {
    await step(index);
    assert.equal(exports.mesh_retained_renders(), 1, `after update ${index}`);
  }
  assert.equal(exports.mesh_live_allocations(), baseline.live);
  assert.equal(exports.memory.buffer.byteLength, baseline.bytes);
  current.release();
  await settle();
  assert.equal(exports.mesh_retained_renders(), 0);
});
