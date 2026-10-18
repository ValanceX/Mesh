// The module stays within its size budget: 300,000 bytes gzipped (outline
// v0.4 D2), raised to 320,000 for enum types, `has`, `??` and prop defaults
// (WP2 step 1), which added about 12,000 (296,029 for 0.13.0, about 309,000
// then), and to 330,000 for snippets (WP2 step 2), which added about 12,000
// more (321,522). The module has no dominant part to trim: the largest
// function is the parser's, at 18 KB raw of 987 KB, and the profile already
// uses opt-level z, lto and strip. The next raise needs a reason of the same kind.
// The build then runs binaryen's wasm-opt (-Oz, inlining off; scripts/build-wasm.mjs): the same
// module went from 989,551 bytes (320,778 gzipped) to 735,048 (301,420), with the budget unchanged.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { shippedModule } from "./common.mjs";

const BUDGET = 330_000;

test("the shipped module is within its gzipped size budget", () => {
  const size = gzipSync(readFileSync(shippedModule), { level: 9 }).length;
  assert.ok(size <= BUDGET, `${size} bytes gzipped, over the ${BUDGET}-byte budget`);
});
