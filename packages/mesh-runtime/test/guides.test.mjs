// The two v0.5 guides' examples run against the slice, and print what
// each guide says they print (outline D10, "It's documented"), as
// "Using MESH from JavaScript" is tested by the compiler's guide.test.mjs.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { packageDir, root } from "./common.mjs";

const packages = {
  "@valancex/mesh-compiler": join(packageDir, "..", "mesh-compiler", "dist", "index.js"),
  "@valancex/mesh-runtime": join(packageDir, "dist", "index.js"),
};

/**
 * Runs the guide's one ```js block, with its imports pointed at the built
 * packages, from the repository's root (so it reads `examples/slice/`),
 * and returns its output and the ```text block that follows it.
 */
function run(name) {
  const guide = readFileSync(join(root, "docs", "guides", name), "utf8");
  const parts = guide.split("```js\n");
  assert.equal(parts.length, 2, `${name} has exactly one js block`);
  let [example, rest] = parts[1].split("\n```\n");
  const expected = rest.split("```text\n")[1]?.split("\n```")[0];
  assert.ok(expected !== undefined, `${name} shows the example's output after it`);
  for (const [specifier, path] of Object.entries(packages)) {
    example = example.replaceAll(JSON.stringify(specifier), JSON.stringify(pathToFileURL(path).href));
  }
  assert.ok(!example.includes("@valancex/"), `${name} imports only the two packages`);
  const dir = mkdtempSync(join(tmpdir(), "mesh-guide-"));
  try {
    const file = join(dir, "example.mjs");
    writeFileSync(file, example);
    const output = execFileSync(process.execPath, [file], { cwd: root, encoding: "utf8" });
    return { output: output.trimEnd(), expected: expected.trimEnd() };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("the NEXUS guide's host runs and prints what the guide says", () => {
  const { output, expected } = run("integrating-mesh-with-nexus.md");
  assert.equal(output, expected);
});

test("the PORT guide's renderer runs and prints what the guide says", () => {
  const { output, expected } = run("rendering-mesh-output.md");
  assert.equal(output, expected);
});
