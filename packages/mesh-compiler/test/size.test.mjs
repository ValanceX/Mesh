// The module stays within its size budget (outline v0.4 D2): 300,000
// bytes gzipped.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { shippedModule } from "./common.mjs";

const BUDGET = 300_000;

test("the shipped module is within its gzipped size budget", () => {
  const size = gzipSync(readFileSync(shippedModule), { level: 9 }).length;
  assert.ok(size <= BUDGET, `${size} bytes gzipped, over the ${BUDGET}-byte budget`);
});
