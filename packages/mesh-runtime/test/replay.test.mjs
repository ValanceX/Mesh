// Replay (Bet 15, R1 to R15) through the WebAssembly package: the session logs in
// `examples/conformance/replay/` and the 1,000 random sessions, played by all three routes
// (an `updateChanges` chain, an `update` chain over whole snapshots, and a `render` of each
// snapshot, never `update`). Every route must give the recorded tree hashes and the recorded
// outcome of every event, and the random sessions must give the digests the native test
// (`crates/mesh-runtime/tests/replay.rs`) recorded: the two runtimes agree.
//
// The player here is the one `vlx replay` is, written test-only: it reads the log's lines and
// nothing else, never runs a command, never reads a clock.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { compile } from "../../mesh-compiler/dist/index.js";
import { dispatch, render, update, updateChanges } from "../dist/index.js";
import { root } from "./common.mjs";

const replay = join(root, "examples", "conformance", "replay");
const read = (...path) => readFileSync(join(replay, ...path), "utf8");
const sha = (text) => createHash("sha256").update(text).digest("hex");

/** The runtime's compact JSON of a tree: members in key order, numbers as MESH writes them. */
function compact(value) {
  if (value === null || typeof value === "boolean") return String(value);
  if (typeof value === "number") return Object.is(value, -0) ? "-0" : String(value);
  if (typeof value === "string") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(compact).join(",")}]`;
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${compact(value[key])}`)
    .join(",")}}`;
}

const hash = (made) => sha(compact(made.tree));
const model = read("components.json");

async function load(name) {
  const program = JSON.parse(read(name, "program.json"));
  const templates = [];
  let sources = "";
  for (const component of program.templates) {
    const source = read(name, `${component}.mprx`);
    const result = await compile({ source, path: `${component}.mprx`, model: { manifest: model, path: "components.json", component } });
    assert.deepEqual(result.diagnostics.diagnostics, [], `${name}/${component}`);
    templates.push(JSON.stringify(result.template));
    sources += `${source}\0`;
  }
  return { root: program.root, templates, programHash: sha(sources), manifestHash: sha(model) };
}

const start = async (loaded, snapshot) => {
  const result = await render({ program: { root: loaded.root, templates: loaded.templates }, model, snapshot: structuredClone(snapshot) });
  assert.ok(result.render, JSON.stringify(result.diagnostics));
  return result.render;
};

/** Applies host-side changes to a plain snapshot. */
function apply(snapshot, changes) {
  for (const change of changes) {
    let at = snapshot;
    for (const step of change.path.slice(0, -1)) at = at[step];
    const last = change.path.at(-1);
    if (change.op === "set") at[last] = structuredClone(change.value);
    else if (change.op === "insert") at.splice(last, 0, structuredClone(change.value));
    else if (Array.isArray(at)) at.splice(last, 1);
    else delete at[last];
  }
}

/** `{ intent }` or `{ diag }`, as the log writes an outcome. */
async function outcome(made, handler, payload) {
  const result = await dispatch(made, handler, payload);
  return result.intent !== undefined ? { intent: result.intent } : { diag: result.diagnostics.diagnostics[0].code };
}

/** Plays a log's lines by one route and returns the tree hashes it gave. */
async function play(name, lines, route) {
  const loaded = await load(name);
  const [header] = lines;
  assert.equal(header.program, loaded.programHash, `${name}: the program is the one recorded`);
  assert.equal(header.manifest, loaded.manifestHash, `${name}: the manifest is the one recorded`);
  let snapshot = structuredClone(header.snapshot);
  let current = await start(loaded, snapshot);
  const renders = [current];
  const hashes = [];
  for (const [index, line] of lines.entries()) {
    if (index === 0) continue;
    const where = `${name} line ${index + 1}: ${route}`;
    if (line.line === "changes") {
      const next = structuredClone(snapshot);
      apply(next, line.changes);
      let result;
      if (route === "changes") result = await updateChanges(current, { base: current.version, changes: line.changes });
      else if (route === "update") result = await update(current, structuredClone(next));
      else result = await render({ program: { root: loaded.root, templates: loaded.templates }, model, snapshot: structuredClone(next) });
      if (line.rejects === undefined) {
        assert.ok(result.render, `${where} rejected: ${JSON.stringify(result.diagnostics)}`);
        snapshot = next;
        current = result.render;
        renders.push(current);
      } else {
        assert.ok(result.diagnostics, `${where} accepted changes to be rejected with ${line.rejects}`);
        assert.equal(result.diagnostics.diagnostics[0].code, line.rejects, where);
      }
    } else if (line.line === "event") {
      assert.deepEqual(await outcome(renders[line.on], line.handler, line.payload), line.out, `${where} dispatches as recorded`);
    } else if (line.line === "tree") {
      assert.equal(hash(current), line.sha256, `${where} gives the recorded tree`);
      if (line.tree !== undefined) assert.deepEqual(current.tree, line.tree, `${where}'s checkpoint`);
      hashes.push(hash(current));
    } else {
      assert.fail(`${where}: a line of kind ${line.line}`);
    }
  }
  return hashes;
}

const names = readdirSync(replay).filter((name) => existsSync(join(replay, name, "scenario.json"))).sort();
const linesOf = (name) => read(name, "log.jsonl").trim().split("\n").map((line) => JSON.parse(line));

test("there are at least six logs", () => {
  assert.ok(names.length >= 6, String(names));
});

for (const name of names) {
  test(`replay ${name}: all three routes give the recorded hashes and outcomes`, async () => {
    const lines = linesOf(name);
    const byChanges = await play(name, lines, "changes");
    assert.deepEqual(await play(name, lines, "update"), byChanges, `${name}: update chain`);
    assert.deepEqual(await play(name, lines, "render"), byChanges, `${name}: render of each snapshot`);
    assert.ok(byChanges.length >= 2);
  });
}

test("a log with a wrong hash does not replay", async () => {
  const lines = linesOf(names[0]);
  lines[1].sha256 = "0".repeat(64);
  await assert.rejects(play(names[0], lines, "changes"));
});

// ---- random sessions: the same generator as the native test, step for step ----------------------------------------------------------

/** mulberry32. */
function generator(seed) {
  let state = seed >>> 0;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1) >>> 0;
    t = (t ^ ((t + Math.imul(t ^ (t >>> 7), t | 61)) >>> 0)) >>> 0;
    return (t ^ (t >>> 14)) >>> 0;
  };
  return { next, below: (n) => next() % n };
}

function session(spec, index) {
  const random = generator((spec.seed + Math.imul(index, 7919)) >>> 0);
  let nextId = 0;
  const fresh = () => {
    nextId += 1;
    return { id: nextId, label: `n${nextId}-${random.below(1000)}` };
  };
  const count = 1 + random.below(6);
  let items = Array.from({ length: count }, fresh);
  const initial = { items: structuredClone(items) };
  const steps = [];
  for (let step = 0; step < spec.steps; step += 1) {
    const kind = items.length === 0 ? 1 : items.length > 10 ? 2 : random.below(5);
    let changes;
    if (kind === 0) {
      const at = random.below(items.length);
      const label = `v${random.below(1000)}`;
      items[at].label = label;
      changes = [{ op: "set", path: ["items", at, "label"], value: label }];
    } else if (kind === 1) {
      const at = random.below(items.length + 1);
      const item = fresh();
      items.splice(at, 0, item);
      changes = [{ op: "insert", path: ["items", at], value: structuredClone(item) }];
    } else if (kind === 2) {
      const at = random.below(items.length);
      items.splice(at, 1);
      changes = [{ op: "remove", path: ["items", at] }];
    } else if (kind === 3) {
      const at = random.below(items.length + 1);
      const item = fresh();
      items.splice(at, 0, item);
      const other = random.below(items.length);
      const label = `w${random.below(1000)}`;
      items[other].label = label;
      changes = [
        { op: "insert", path: ["items", at], value: structuredClone(item) },
        { op: "set", path: ["items", other, "label"], value: label },
      ];
    } else {
      const k = random.below(5);
      items = Array.from({ length: k }, fresh);
      changes = [{ op: "set", path: ["items"], value: structuredClone(items) }];
    }
    steps.push(changes);
  }
  return { initial, steps };
}

test("a thousand random sessions agree by the three routes, and with the digests the native runtime recorded", async () => {
  const spec = JSON.parse(read("random.expected.json"));
  const loaded = await load(spec.program);
  const whole = { program: { root: loaded.root, templates: loaded.templates }, model };
  const digests = [];
  for (let index = 0; index < spec.sessions; index += 1) {
    const { initial, steps } = session(spec, index);
    const byRoute = {};
    for (const route of ["changes", "update", "render"]) {
      let snapshot = structuredClone(initial);
      let current = await start(loaded, snapshot);
      const hashes = [hash(current)];
      for (const changes of steps) {
        apply(snapshot, changes);
        if (route === "changes") current = (await updateChanges(current, { base: current.version, changes })).render;
        else if (route === "update") current = (await update(current, structuredClone(snapshot))).render;
        else current = (await render({ ...whole, snapshot: structuredClone(snapshot) })).render;
        assert.ok(current, `session ${index} ${route}`);
        hashes.push(hash(current));
      }
      byRoute[route] = sha(hashes.join("\n")).slice(0, 16);
    }
    assert.equal(byRoute.update, byRoute.changes, `session ${index}: update chain`);
    assert.equal(byRoute.render, byRoute.changes, `session ${index}: render of each snapshot`);
    digests.push(byRoute.changes);
  }
  assert.deepEqual(digests, spec.digests, "the JavaScript runtime gives the digests the native runtime recorded");
});
