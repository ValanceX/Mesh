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
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { compile } from "../../mesh-compiler/dist/index.js";
import { dispatch, render } from "../dist/index.js";
import { root, validateTree } from "./common.mjs";

const conformance = join(root, "examples", "conformance");
const read = (...path) => readFileSync(join(conformance, ...path), "utf8");
const model = read("components.json");

async function rendered(name) {
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
    snapshot: JSON.parse(read(name, "snapshot.json")),
  });
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
