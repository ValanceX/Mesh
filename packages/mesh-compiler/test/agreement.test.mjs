// Every batch host answers what the compiler's check answers (outline
// v0.4 I7): for every corpus run, and for seeded mutants, the API's
// document equals `mesh check --format json`'s, parsed.
//
// A longer run: MESH_FUZZ_ITERATIONS=100000 MESH_FUZZ_SEED=1 npm test
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { check } from "../dist/index.js";
import { cli, corpus, input, mutate, prng, read } from "./common.mjs";

test("every corpus run agrees with mesh check", async () => {
  for (const run of corpus()) {
    assert.deepEqual(await check(input(run)), cli(run.file, run.model), JSON.stringify(run));
  }
});

test("seeded mutants agree with mesh check", async () => {
  const iterations = Number(process.env.MESH_FUZZ_ITERATIONS ?? 300);
  const seed = Number(process.env.MESH_FUZZ_SEED ?? 0x4d455348);
  const random = prng(seed);
  const runs = corpus().filter((run) => !run.model || run.model.manifest === "components.json");
  const dir = mkdtempSync(join(tmpdir(), "mesh-agreement-"));
  try {
    const started = Date.now();
    for (let index = 0; index < iterations; index++) {
      const run = runs[Math.floor(random() * runs.length)];
      let source = read(run.file);
      const rounds = 1 + Math.floor(random() * 3);
      for (let round = 0; round < rounds; round++) {
        source = mutate(source, random);
      }
      writeFileSync(join(dir, "mutant.mprx"), source);
      const model = run.model && { manifest: join(dir, "components.json"), component: run.model.component };
      if (model) {
        writeFileSync(model.manifest, read(run.model.manifest));
      }
      const api = await check({
        source,
        path: "mutant.mprx",
        model: model && { manifest: read(run.model.manifest), path: model.manifest, component: model.component },
      });
      assert.deepEqual(api, cli("mutant.mprx", model, dir), `seed ${seed}, mutant ${index}: ${JSON.stringify(source)}`);
    }
    if (iterations >= 10000) {
      console.log(`${iterations} mutants agreed (seed ${seed}) in ${Date.now() - started} ms`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
