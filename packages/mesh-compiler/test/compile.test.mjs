// compile() (v0.5 D2, I7): the same diagnostics and template as
// `mesh compile`, for every corpus run with a model.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import Ajv2020 from "ajv/dist/2020.js";
import { compile, check } from "../dist/index.js";
import { cliCompile, corpus, input, root } from "./common.mjs";

const runs = corpus().filter((run) => run.model);

test("compile() agrees with mesh compile on every run with a model", async () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-compile-"));
  try {
    let templates = 0;
    for (const run of runs) {
      const expected = cliCompile(run.file, run.model, dir);
      const result = await compile(input(run));
      assert.deepEqual(result.diagnostics, expected.diagnostics, run.file);
      assert.deepEqual(result.template, expected.template, run.file);
      assert.equal("template" in result, expected.template !== undefined, `${run.file}: no template property without a template`);
      if (result.template) templates += 1;
    }
    assert.ok(templates >= 20, `only ${templates} templates`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("compile() reports exactly what check() reports", async () => {
  for (const run of runs) {
    const value = input(run);
    assert.deepEqual((await compile(value)).diagnostics, await check(value), run.file);
  }
});

test("compiling is deterministic", async () => {
  for (const run of runs) {
    const value = input(run);
    assert.deepEqual(await compile(value), await compile(value), run.file);
  }
});

test("every template matches template-v1", async () => {
  const schema = JSON.parse(readFileSync(join(root, "schemas", "template-v1.schema.json"), "utf8"));
  const validate = new Ajv2020({ allErrors: true, strict: false }).compile(schema);
  for (const run of runs) {
    const { template } = await compile(input(run));
    if (template) {
      assert.ok(validate(template), `${run.file}: ${JSON.stringify(validate.errors)}`);
    }
  }
});

test("compile() needs a model", async () => {
  await assert.rejects(compile({ source: "<a />", path: "a.mprx" }), TypeError);
  await assert.rejects(compile({ source: "<a />", path: "a.mprx", model: null }), TypeError);
  await assert.rejects(compile(null), TypeError);
});
