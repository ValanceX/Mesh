// The runtime module stays within its size budget (Pass 3 Decision 12):
// its measured size when the package was made, 184,751 bytes gzipped,
// plus 50%, and never over the compiler's 300,000. (The build then runs binaryen: 248,984 gzipped become 234,818.)
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

// The linker's output is about 257,000 gzipped and the optimized build about 243,000 (units added about 3.5 percent to both; before them 248,984 and 234,818): a module over this was not run through
// wasm-opt (`MESH_NO_WASM_OPT` is for a build that must be the linker's output, and is not what ships).
test("the shipped module is the optimized build", () => {
  const size = gzipSync(readFileSync(shippedModule), { level: 9 }).length;
  assert.ok(size <= 250_000, `${size} bytes gzipped: it looks like the linker's output, not the optimized module`);
});

test("the module imports nothing", async () => {
  const module = await WebAssembly.compile(readFileSync(shippedModule));
  assert.deepEqual(WebAssembly.Module.imports(module), []);
});
