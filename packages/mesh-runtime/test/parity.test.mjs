// Parity (I11): the WebAssembly module and the native runtime give
// byte-identical results for the same inputs, as the same bytes. Every
// request goes through the shipped module (the test module for number
// text) and through the native harness (`mesh-runtime-wasm`'s example).
//
// The requests: every call the native property test made (it writes them
// to `target/mesh-runtime-property/`: run `cargo test -p mesh-runtime
// --test property` first), the test programs' renders and events, every
// kind of JavaScript value, randomly built JavaScript snapshots, the
// slice, and the normative number-to-text table.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { dispatchRequest, Module, numbersRequest, renderRequest } from "./common/module.mjs";
import {
  MODEL,
  programDirs,
  programTemplates,
  prng,
  root,
  shippedModule,
  SNAPSHOT,
  startHarness,
  template,
  testModule,
} from "./common.mjs";

const shipped = new Module(shippedModule);
const hooked = new Module(testModule);

/** Runs every request both ways; returns how many. */
async function agree(requests, label) {
  const harness = startHarness();
  try {
    for (const request of requests) {
      const native = await harness.ask(request);
      const module = request.op === "numbers" ? hooked : shipped;
      const wasm = module.request(request);
      if (native.status !== wasm.status || native.result !== wasm.result) {
        assert.fail(
          `${label}: ${JSON.stringify(request).slice(0, 300)}\nnative: ${JSON.stringify(native).slice(0, 500)}\nwasm:   ${JSON.stringify(wasm).slice(0, 500)}`,
        );
      }
    }
  } finally {
    await harness.close();
  }
  return requests.length;
}

/** Each handler in a tree, from a render's result text. */
function handlersOf(result) {
  const out = [];
  const walk = (node) => {
    out.push(...Object.values(node.events));
    for (const child of node.children) if (child.type === "node") walk(child);
  };
  const document = JSON.parse(result);
  if (document.tree) walk(document.tree.root);
  return out;
}

test("the native property test's calls", async () => {
  const dir = join(root, "target", "mesh-runtime-property");
  assert.ok(
    existsSync(dir),
    `no ${dir}: run \`cargo test -p mesh-runtime --test property\` first`,
  );
  let total = 0;
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".jsonl")).sort()) {
    const requests = readFileSync(join(dir, file), "utf8")
      .split("\n")
      .filter(Boolean)
      .map((line) => JSON.parse(line));
    total += await agree(requests, file);
  }
  assert.ok(total >= 1000, `only ${total} requests`);
  console.log(`parity: ${total} property requests`);
});

test("the test programs' renders and events", async () => {
  const requests = [];
  for (const dir of programDirs()) {
    const templates = await programTemplates(dir);
    const snapshots = readdirSync(join(dir, "snapshots")).filter((f) => f.endsWith(".json")).sort();
    for (const file of snapshots) {
      const render = renderRequest("view", templates, MODEL, JSON.parse(readFileSync(join(dir, "snapshots", file), "utf8")));
      requests.push(render);
      const handlers = handlersOf(shipped.request(render).result);
      const payloads = [undefined, null, { x: 3, y: -0 }, { any: ["thing", 1] }, { x: "three" }, "Countess"];
      for (const handler of handlers) {
        for (const payload of payloads) requests.push(dispatchRequest(render, handler, payload));
      }
    }
  }
  console.log(`parity: ${await agree(requests, "programs")} program requests`);
});

test("the slice", async () => {
  const slice = join(root, "examples", "slice");
  const read = (name) => readFileSync(join(slice, name), "utf8");
  const model = read("components.json");
  const templates = [];
  for (const component of ["users", "user-card"]) {
    templates.push(await template(component, read(`${component}.mprx`), model));
  }
  const requests = [];
  for (const file of ["first.json", "second.json"]) {
    const render = renderRequest("users", templates, model, JSON.parse(read(`snapshots/${file}`)));
    requests.push(render);
    for (const handler of handlersOf(shipped.request(render).result)) {
      for (const payload of [undefined, { x: 12, y: 34 }, { x: 1 }]) {
        requests.push(dispatchRequest(render, handler, payload));
      }
    }
  }
  console.log(`parity: ${await agree(requests, "slice")} slice requests`);
});

class Point {
  constructor() {
    this.x = 1;
  }
}

/** A random JavaScript value, of every kind a host might give. */
function randomValue(random, depth = 0) {
  const pick = (items) => items[Math.floor(random() * items.length)];
  const kinds = depth > 2 ? 12 : 16;
  switch (Math.floor(random() * kinds)) {
    case 0: return null;
    case 1: return random() < 0.5;
    case 2: return pick([0, -0, 1, -1.5, 0.1, 1e21, 5e-324, Number.MAX_VALUE, NaN, Infinity, -Infinity]);
    case 3: return new DataView(new Float64Array([random() * 2 ** 32]).buffer).getFloat64(0);
    case 4: return pick(["", "Ada", "😀", "\ud800", "a\udc00b", "line\nbreak"]);
    case 5: return undefined;
    case 6: return 10n;
    case 7: return () => 1;
    case 8: return Symbol("s");
    case 9: return pick([new Map(), new Date(0), new Point(), Object.create(null)]);
    case 10: return pick([1, "two", true]);
    case 11: return { name: "Ada", active: true };
    case 12: {
      // eslint-disable-next-line no-sparse-arrays
      const list = Array.from({ length: Math.floor(random() * 4) }, () => randomValue(random, depth + 1));
      if (random() < 0.2) list.length += 1; // a hole at the end
      return list;
    }
    case 13: {
      const record = {};
      for (const name of ["name", "active", "avatar", "x", "\ud800"]) {
        if (random() < 0.5) record[name] = randomValue(random, depth + 1);
      }
      return record;
    }
    case 14: {
      const self = { name: "Ada" };
      self.self = self;
      return self;
    }
    default:
      return [randomValue(random, depth + 1)];
  }
}

test("every kind of JavaScript value, and random JavaScript snapshots", async () => {
  const templates = [await template("view", "<page title={name}><text>{count} {anything}</text><probe num={count} on.tap={save()} /></page>")];
  const random = prng(Number(process.env.MESH_PARITY_SEED ?? 0x5eed));
  const total = Number(process.env.MESH_PARITY_SNAPSHOTS ?? 2000);
  const requests = [];
  const names = Object.keys(SNAPSHOT).concat(["maybeName", "maybeUser", "maybeAnything", "undeclared"]);
  for (let index = 0; index < total; index++) {
    const snapshot = { ...SNAPSHOT };
    for (let changes = 1 + Math.floor(random() * 3); changes > 0; changes--) {
      snapshot[names[Math.floor(random() * names.length)]] = randomValue(random);
    }
    const render = renderRequest("view", templates, MODEL, snapshot);
    requests.push(render);
    for (const handler of handlersOf(shipped.request(render).result)) {
      requests.push(dispatchRequest(render, handler, random() < 0.5 ? undefined : randomValue(random)));
    }
  }
  console.log(`parity: ${await agree(requests, "random")} random requests`);
});

test("the normative number-to-text table, through WebAssembly", async () => {
  const rows = readFileSync(join(root, "docs", "tables", "number-to-text.tsv"), "utf8")
    .split("\n")
    .slice(1)
    .filter(Boolean)
    .map((line) => line.split("\t"))
    .map(([bits, expected, category, why]) => ({ bits, expected, category, why }));
  const finite = rows.filter((row) => !row.expected.startsWith("!"));
  const numbers = finite.map((row) => new DataView(new BigUint64Array([BigInt(`0x${row.bits}`)]).buffer).getFloat64(0, true));
  const { status, result } = hooked.request(numbersRequest(numbers));
  assert.equal(status, 0);
  const texts = result.split("\n").slice(0, -1);
  finite.forEach((row, index) => {
    assert.equal(texts[index], row.expected, `${row.bits} [${row.category}] ${row.why}`);
  });
  assert.ok(finite.length >= 80, `only ${finite.length} finite rows`);
  await agree([numbersRequest(numbers)], "table");
});
