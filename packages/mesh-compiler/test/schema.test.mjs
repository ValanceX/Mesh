// Every document the API returns is one the published schema accepts.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import Ajv2020 from "ajv/dist/2020.js";
import { check } from "../dist/index.js";
import { corpus, input, root } from "./common.mjs";

test("the schema accepts every document", async () => {
  const schema = JSON.parse(readFileSync(join(root, "schemas", "diagnostics-v1.schema.json"), "utf8"));
  const validate = new Ajv2020({ allErrors: true, strict: false }).compile(schema);
  for (const run of corpus()) {
    const document = await check(input(run));
    assert.ok(validate(document), `${run.file}: ${JSON.stringify(validate.errors)}`);
  }
});
