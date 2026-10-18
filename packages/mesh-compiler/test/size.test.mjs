// The module stays within its size budget: 300,000 bytes gzipped (outline
// v0.4 D2), raised to 320,000 for enum types, `has`, `??` and prop defaults
// (WP2 step 1), which added about 12,000 (296,029 for 0.13.0, about 309,000
// now). The next raise needs a reason of the same kind.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { shippedModule } from "./common.mjs";

const BUDGET = 320_000;

test("the shipped module is within its gzipped size budget", () => {
  const size = gzipSync(readFileSync(shippedModule), { level: 9 }).length;
  assert.ok(size <= BUDGET, `${size} bytes gzipped, over the ${BUDGET}-byte budget`);
});
