// checkProgram() (v0.5 D2, D7): for every case of
// `crates/mesh-cli/tests/check-program/`, the document `mesh
// check-program --format json` prints (committed there as
// `expected.json`, which the CLI's own test checks against the runtime),
// with the templates compiled here by compile().
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import Ajv2020 from "ajv/dist/2020.js";
import { checkProgram, compile } from "../dist/index.js";
import { root } from "./common.mjs";

const cases = join(root, "crates", "mesh-cli", "tests", "check-program");
const shared = readFileSync(join(cases, "components.json"), "utf8");

function names() {
  return readdirSync(cases)
    .filter((name) => statSync(join(cases, name)).isDirectory())
    .sort();
}

/** A case's program: its root, its templates (compiled or as written) and its model. */
async function program(name) {
  const dir = join(cases, name);
  const spec = JSON.parse(readFileSync(join(dir, "program.json"), "utf8"));
  const templates = [];
  for (const entry of spec.templates) {
    const source = typeof entry === "string" ? entry : entry.source;
    const text = readFileSync(join(dir, source), "utf8");
    if (!source.endsWith(".mprx")) {
      templates.push(text);
      continue;
    }
    const component = source.slice(0, -".mprx".length);
    const manifest = typeof entry === "string" || !entry.model ? shared : readFileSync(join(dir, entry.model), "utf8");
    const result = await compile({ source: text, path: source, model: { manifest, path: "components.json", component } });
    assert.ok(result.template, `${name}/${source} compiles`);
    templates.push(JSON.stringify(result.template));
  }
  const model = spec.checkModel ? readFileSync(join(dir, spec.checkModel), "utf8") : shared;
  return { model, root: spec.root, templates };
}

test("checkProgram() equals mesh check-program for every case", async () => {
  const all = names();
  assert.ok(all.length >= 12, `only ${all.length} cases`);
  for (const name of all) {
    const expected = JSON.parse(readFileSync(join(cases, name, "expected.json"), "utf8"));
    assert.deepEqual(await checkProgram(await program(name)), expected, name);
  }
});

test("its documents match runtime-diagnostics-v1", async () => {
  const schema = JSON.parse(readFileSync(join(root, "schemas", "runtime-diagnostics-v1.schema.json"), "utf8"));
  const validate = new Ajv2020({ strict: false }).compile(schema);
  for (const name of names()) {
    const document = await checkProgram(await program(name));
    assert.ok(validate(document), `${name}: ${JSON.stringify(validate.errors)}`);
  }
});

test("checkProgram() refuses arguments of the wrong type", async () => {
  await assert.rejects(checkProgram(null), TypeError);
  await assert.rejects(checkProgram({ model: "{}", root: "view" }), TypeError);
  await assert.rejects(checkProgram({ model: "{}", root: 1, templates: [] }), TypeError);
  await assert.rejects(checkProgram({ model: "{}", root: "view", templates: [1] }), TypeError);
});

test("an empty program has no root", async () => {
  const document = await checkProgram({ model: shared, root: "view", templates: [] });
  assert.deepEqual(document.diagnostics.map((d) => d.code), ["assembly-missing-root"]);
});
