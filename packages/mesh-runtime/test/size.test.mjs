// The runtime module stays within its size budget (Pass 3 Decision 12):
// its measured size when the package was made, 184,751 bytes gzipped,
// plus 50%, and never over the compiler's 300,000.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { gzipSync } from "node:zlib";
import { shippedModule } from "./common.mjs";

const BUDGET = Math.min(Math.floor(184_751 * 1.5), 300_000);

test("the shipped module is within its gzipped size budget", () => {
  const size = gzipSync(readFileSync(shippedModule), { level: 9 }).length;
  assert.ok(size <= BUDGET, `${size} bytes gzipped, over the ${BUDGET}-byte budget`);
});

test("the module imports nothing", async () => {
  const module = await WebAssembly.compile(readFileSync(shippedModule));
  assert.deepEqual(WebAssembly.Module.imports(module), []);
});
