// The conformance vectors (`examples/conformance/`), in Node, through the
// package's public API, against the same committed files the native test
// (`crates/mesh-runtime/tests/conformance.rs`) blessed.
//
// Values: the tree `render` gives is the committed one, value for value,
// and each case's prop and `propText` entry is as committed.
//
// Events: event resolution (spec §9.9) is a rule a renderer implements
// for its own target, from the tree alone. `resolve` below is that rule
// as the spec states it, written here, test-only, with nothing from the
// package: it shows the vectors can be met without MESH's code, and
// without any target. Each resolved handler is dispatched through the
// package, once, and gives the committed intent.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { compile } from "../../mesh-compiler/dist/index.js";
import { dispatch, render, update } from "../dist/index.js";
import { root, validateTree } from "./common.mjs";

const conformance = join(root, "examples", "conformance");
const read = (...path) => readFileSync(join(conformance, ...path), "utf8");
const shared = read("components.json");
// A program's own manifest if its directory has one (`enums/` does), else the shared one.
const modelOf = (name) => (existsSync(join(conformance, name, "components.json")) ? read(name, "components.json") : shared);

async function build(name, snapshot = JSON.parse(read(name, "snapshot.json")), units = undefined) {
  const model = modelOf(name);
  const program = JSON.parse(read(name, "program.json"));
  const templates = [];
  for (const component of program.templates) {
    const result = await compile({
      source: read(name, `${component}.mprx`),
      path: `${component}.mprx`,
      model: { manifest: model, path: "components.json", component },
    });
    assert.deepEqual(result.diagnostics.diagnostics, [], component);
    templates.push(JSON.stringify(result.template));
  }
  const result = await render({
    program: { root: program.root, templates },
    model,
    snapshot,
    ...(units === undefined ? {} : { units }),
  });
  return result;
}

async function rendered(name) {
  const result = await build(name);
  assert.ok(result.render, JSON.stringify(result.diagnostics));
  const tree = result.render.tree;
  // By value: JSON.parse reads each committed number as its exact binary64
  // value, and deepEqual compares numbers with Object.is.
  assert.deepEqual(tree, JSON.parse(read(name, "expected.tree.json")));
  assert.ok(validateTree(tree), JSON.stringify(validateTree.errors));
  return result.render;
}

function nodes(node, out = new Map()) {
  out.set(node.key, node);
  for (const child of node.children) if (child.type === "node") nodes(child, out);
  return out;
}

/** `{ value }` or `{ absent: true }` for a member that may be missing. */
const entry = (object, name, as) =>
  object !== undefined && Object.hasOwn(object, name) ? { [as]: object[name] } : { absent: true };

test("values: props and propText as committed", async () => {
  const { tree } = await rendered("values");
  const all = nodes(tree.root);
  for (const item of JSON.parse(read("values", "cases.json"))) {
    if (item.states) {
      // Each state's node is exactly as given: its props, and its
      // propText member only when the tree has one. No two are alike.
      const written = item.states.map(({ node }) => {
        const found = all.get(node);
        return Object.hasOwn(found, "propText")
          ? { props: found.props, propText: found.propText }
          : { props: found.props };
      });
      item.states.forEach((state, index) => assert.deepEqual(written[index], state.exactly, `${item.case}: ${state.state}`));
      for (let a = 0; a < written.length; a++) {
        for (let b = a + 1; b < written.length; b++) {
          assert.notDeepEqual(written[a], written[b], `${item.case}: ${item.states[a].state} and ${item.states[b].state}`);
        }
      }
      continue;
    }
    if (item.textRun) {
      const run = [...all.values()].flatMap((node) => node.children).find((child) => child.key === item.textRun);
      assert.equal(run.text, item.text, item.case);
      continue;
    }
    const node = all.get(item.node);
    assert.ok(node, item.case);
    assert.deepEqual(entry(node.props, item.prop, "value"), item.value, item.case);
    assert.deepEqual(entry(node.propText, item.prop, "text"), item.propText, item.case);
  }
});

test("values: a list or record never has text; absent and null stay apart", async () => {
  const { tree } = await rendered("values");
  for (const node of nodes(tree.root).values()) {
    for (const [prop, value] of Object.entries(node.props)) {
      const text = node.propText?.[prop];
      if (typeof value === "string" || typeof value === "object" && value !== null) {
        assert.equal(text, undefined, `${node.component}.${prop}`);
      } else {
        assert.equal(typeof text, "string", `${node.component}.${prop}`);
      }
    }
    for (const prop of Object.keys(node.propText ?? {})) {
      assert.ok(Object.hasOwn(node.props, prop), "no text without a prop");
    }
  }
});

/**
 * Event resolution, as spec §9.9 states it: from the innermost interacted
 * node (a text run's is its parent) towards the root, the first node whose
 * primitive has an applicable event and which binds it receives the
 * interaction, and nothing after it.
 */
function resolve(tree, { target, applicable }) {
  const path = [];
  const find = (node) => {
    path.push(node);
    if (node.key === target) return true;
    for (const child of node.children) {
      if (child.type === "text" ? child.key === target : find(child)) return true;
    }
    path.pop();
    return false;
  };
  if (!find(tree.root)) throw new Error(`no ${target} in the tree`);
  for (const node of path.reverse()) {
    const event = applicable[node.component];
    if (event !== undefined && Object.hasOwn(node.events, event)) {
      return { node: node.key, event, handler: node.events[event] };
    }
  }
  return null;
}

test("events: each interaction resolves to its committed binding, and one intent", async () => {
  const kept = await rendered("events");
  for (const item of JSON.parse(read("events", "cases.json"))) {
    const resolved = resolve(kept.tree, item.interaction);
    if (resolved === null) {
      assert.deepEqual({ none: true }, item.expect, item.case);
      continue;
    }
    const { intent, diagnostics } = await dispatch(kept, resolved.handler);
    assert.equal(diagnostics, undefined, item.case);
    assert.deepEqual({ ...resolved, intent }, item.expect, item.case);
  }
});

// `arguments/`: an intent's argument names. Each case is an event on a node, with a payload or none,
// and the intent it gives: an argument written as a plain path carries its last segment as `name`.
test("arguments: each argument written as a path carries its name, as committed", async () => {
  const kept = await rendered("arguments");
  const all = nodes(kept.tree.root);
  for (const item of JSON.parse(read("arguments", "cases.json"))) {
    const { intent, diagnostics } = await dispatch(kept, all.get(item.node).events[item.event], item.payload);
    assert.equal(diagnostics, undefined, item.case);
    assert.deepEqual(intent, item.intent, item.case);
  }
});

// `enums/`: the first program with its own manifest. Its cases are
// `{ props, rejects, dispatches }`: a node's props; snapshots that don't
// render, with the first diagnostic's code and location; and events with a
// payload, with the intent they give or the diagnostic.
const enumCases = () => JSON.parse(read("enums", "cases.json"));

test("enums: props are as committed", async () => {
  const kept = await rendered("enums");
  const all = nodes(kept.tree.root);
  for (const item of enumCases().props) {
    assert.deepEqual(entry(all.get(item.node).props, item.prop, "value"), item.value, item.case);
  }
});

test("enums: a value that is not a variant is refused at the boundary", async () => {
  for (const item of enumCases().rejects) {
    const result = await build("enums", item.snapshot);
    assert.equal(result.render, undefined, item.case);
    const [first] = result.diagnostics.diagnostics;
    assert.deepEqual({ code: first.code, location: first.location }, { code: item.code, location: item.location }, item.case);
  }
});

test("enums: an event whose payload or argument is not a variant is refused", async () => {
  const kept = await rendered("enums");
  const all = nodes(kept.tree.root);
  for (const item of enumCases().dispatches) {
    const handler = all.get(item.node).events[item.event];
    const result = await dispatch(kept, handler, item.payload);
    const actual = result.intent
      ? { intent: result.intent }
      : { code: result.diagnostics.diagnostics[0].code, location: result.diagnostics.diagnostics[0].location };
    assert.deepEqual(actual, item.expect, item.case);
  }
});

// `snippets/`: typed template values (decisions U1 to U14). The program of its own manifest is
// rendered for `snapshot.json` and for each `step-N.snapshot.json` after it, each against its committed
// tree; updating from a step to the next is the render of the next; and `cases.json` relates the keys of
// nodes (a node is named by the indexes of the nodes, not text runs, on the way down from the root).
const snippetSteps = (dir) => {
  const steps = [{ snapshot: "snapshot.json", expected: "expected.tree.json" }];
  for (let at = 1; existsSync(join(conformance, dir, `step-${at}.snapshot.json`)); at += 1) {
    steps.push({ snapshot: `step-${at}.snapshot.json`, expected: `step-${at}.expected.tree.json` });
  }
  return steps;
};

const snippetRender = async (dir, step) => {
  const result = await build(dir, JSON.parse(read(dir, step.snapshot)));
  assert.ok(result.render, JSON.stringify(result.diagnostics));
  assert.deepEqual(result.render.tree, JSON.parse(read(dir, step.expected)), step.snapshot);
  return result.render;
};

for (const dir of ["snippets", "forwarder", "passing"]) {
test(`${dir}: every step renders its committed tree, and an update is a render`, async () => {
  const steps = snippetSteps(dir);
  assert.ok(steps.length >= 3);
  let previous = await snippetRender(dir, steps[0]);
  for (const step of steps.slice(1)) {
    const fresh = await snippetRender(dir, step);
    const updated = await update(previous, JSON.parse(read(dir, step.snapshot)));
    assert.ok(updated.render, JSON.stringify(updated.diagnostics));
    assert.deepEqual(updated.render.tree, fresh.tree, `an update to ${step.snapshot} is its render`);
    previous = updated.render;
  }
});

test(`${dir}: keys are distinct, the same, or different, as committed`, async () => {
  const steps = snippetSteps(dir);
  const trees = [];
  for (const step of steps) trees.push((await snippetRender(dir, step)).tree);
  const nodeAt = (tree, path) => {
    let node = tree.root;
    for (const index of path) {
      node = node.children.filter((child) => child.type === "node")[index];
      assert.ok(node, `no node ${JSON.stringify(path)}`);
    }
    return node.key;
  };
  const cases = JSON.parse(read(dir, "cases.json"));
  for (const item of cases.distinct) {
    const keys = item.paths.map((path) => nodeAt(trees[item.step], path));
    assert.equal(new Set(keys).size, keys.length, item.case);
  }
  const at = (side) => nodeAt(trees[side.step], side.path);
  for (const item of cases.same) assert.equal(at(item.a), at(item.b), item.case);
  for (const item of cases.different) assert.notEqual(at(item.a), at(item.b), item.case);
});

test(`${dir}: an event in a snippet's body is the declaring template's, and dispatches`, async () => {
  const kept = await snippetRender(dir, snippetSteps(dir)[0]);
  const all = nodes(kept.tree.root);
  const press = [...all.values()].find((node) => node.component === "row" && node.props.label === "Grace");
  const { intent, diagnostics } = await dispatch(kept, press.events.press);
  assert.equal(diagnostics, undefined);
  assert.deepEqual(intent, { command: { component: "screen", name: "select" }, arguments: [{ value: 2, name: "id" }] });
});
}

// `units/`: a component's state for one occurrence, which a render takes beside the snapshot. Each step of `steps.json` is rendered with its
// units against the committed tree and the committed list of occurrences, and updating from a step to the next with the next's units is the
// render of the next. The units are the vectors' `[ { occurrence, state } ]`, given to the package as an object of states by occurrence.
const unitStates = (units) => Object.fromEntries(units.map((each) => [each.occurrence, each.state]));
const unitSteps = () => JSON.parse(read("units", "steps.json"));

test("units: every step renders its committed tree and lists its committed occurrences, and an update with the next units is a render", async () => {
  let previous;
  for (const [at, step] of unitSteps().entries()) {
    const result = await build("units", step.snapshot, unitStates(step.units));
    assert.ok(result.render, JSON.stringify(result.diagnostics));
    assert.deepEqual(result.render.tree, JSON.parse(read("units", `step-${at}.expected.tree.json`)), step.case);
    assert.deepEqual(result.render.occurrences, JSON.parse(read("units", `step-${at}.expected.occurrences.json`)), step.case);
    if (previous !== undefined) {
      const updated = await update(previous, step.snapshot, unitStates(step.units));
      assert.ok(updated.render, JSON.stringify(updated.diagnostics));
      assert.deepEqual(updated.render.tree, result.render.tree, `an update to step ${at} is its render`);
      assert.deepEqual(updated.render.occurrences, result.render.occurrences, `and lists what it lists`);
    }
    previous = result.render;
  }
});
