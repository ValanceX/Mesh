// The nesting limits hold in WebAssembly, at its default stack (outline
// v0.4 D6): `crates/mesh-compiler/tests/nesting.rs`'s sources, nested
// exactly to the limit, give diagnostics and no `nesting-too-deep`; one
// level more gives exactly one `nesting-too-deep`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { check } from "../dist/index.js";

const MAX = 128;

const MANIFEST = JSON.stringify({
  version: 1,
  types: {},
  components: {
    a: {
      props: { x: { type: { kind: "any" }, required: false } },
      events: { e: { payload: { kind: "any" } } },
      commands: { f: { parameters: [{ name: "v", type: { kind: "any" } }] } },
      scope: { y: { kind: "any" } },
    },
  },
});

const model = { manifest: MANIFEST, path: "m.json", component: "a" };

function sources() {
  const p = MAX - 2;
  const attribute = (expression) => `<a x={${expression}} />`;
  return [
    "<a>".repeat(MAX) + "</a>".repeat(MAX),
    attribute("(".repeat(p) + "y" + ")".repeat(p)),
    attribute("!".repeat(p) + "y"),
    attribute("-".repeat(p) + "y"),
    attribute("y" + ".a".repeat(p)),
    attribute("[".repeat(p) + "y" + "]".repeat(p)),
    attribute("{k: ".repeat(p) + "y" + "}".repeat(p)),
    attribute("y" + " + y".repeat(p)),
    attribute("y" + " == y".repeat(p)),
    attribute("y ? y : ".repeat(p) + "y"),
    `<a on.e={${"f(".repeat(p)}$event${")".repeat(p)}} />`,
    attribute("[".repeat(p) + "usr" + "]".repeat(p)),
    "<a>".repeat(MAX / 2) + `<a x={${"!".repeat(MAX / 2 - 3)}y} />` + "</a>".repeat(MAX / 2),
  ];
}

test("every way of nesting, to the limit, checks", async () => {
  for (const [index, source] of sources().entries()) {
    const document = await check({ source, path: "deep.mprx", model });
    const codes = document.diagnostics.map((diagnostic) => diagnostic.code);
    assert.ok(!codes.includes("nesting-too-deep"), `source ${index}: ${codes}`);
    assert.ok(!codes.includes("syntax-error"), `source ${index}: ${codes}`);
  }
});

test("one level past the limit is one nesting-too-deep", async () => {
  const source = "<a>".repeat(MAX + 1) + "</a>".repeat(MAX + 1);
  const document = await check({ source, path: "deep.mprx", model });
  assert.deepEqual(document.diagnostics.map((diagnostic) => diagnostic.code), ["nesting-too-deep"]);
});

test("far past the limit is still one nesting-too-deep", async () => {
  for (const depth of [5_000, 100_000]) {
    const source = `<a x={${"[".repeat(depth)}${"]".repeat(depth)}} />`;
    const document = await check({ source, path: "deep.mprx" });
    assert.deepEqual(document.diagnostics.map((diagnostic) => diagnostic.code), ["nesting-too-deep"], `${depth}`);
  }
});

test("a manifest type nested to the limit checks", async () => {
  const levels = MAX - 3;
  const deep = (kind) => '{ "kind": "list", "element": '.repeat(levels) + `{ "kind": "${kind}" }` + " }".repeat(levels);
  const manifest = `{ "version": 1, "types": { "Strings": ${deep("string")}, "Numbers": ${deep("number")} },
    "components": { "a": {
      "props": { "x": { "type": { "kind": "named", "name": "Strings" }, "required": true } },
      "events": {}, "commands": {},
      "scope": { "s": { "kind": "named", "name": "Strings" }, "n": { "kind": "named", "name": "Numbers" } }
    } } }`;
  const document = await check({
    source: "<a x={s}><a x={n} /><a x={s == n ? s : s} /></a>",
    path: "deep.mprx",
    model: { manifest, path: "m.json", component: "a" },
  });
  assert.deepEqual(document.diagnostics.map((diagnostic) => diagnostic.code), ["type-mismatch", "no-common-type"]);
});
