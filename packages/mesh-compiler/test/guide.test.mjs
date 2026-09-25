// The "Using MESH from JavaScript" guide's example runs, and prints what
// the guide says it prints (outline v0.4's "It's usable").
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { packageDir, root } from "./common.mjs";

const guide = readFileSync(join(root, "docs", "guides", "using-mesh-from-javascript.md"), "utf8");

/** The fenced block of `language` right after the heading `heading`. */
function block(heading, language) {
  const section = guide.split(`## ${heading}\n`)[1];
  assert.ok(section, `the guide has a "${heading}" section`);
  const body = section.split("```" + language + "\n")[1];
  assert.ok(body, `"${heading}" has a ${language} block`);
  return body.split("\n```")[0];
}

test("the guide's check-repair loop runs and prints what the guide says", () => {
  const example = block("Check, repair, check again", "js");
  const expected = block("Check, repair, check again", "text");
  const index = pathToFileURL(join(packageDir, "dist", "index.js")).href;
  assert.ok(example.includes('from "@valancex/mesh-compiler"'));
  const dir = mkdtempSync(join(tmpdir(), "mesh-guide-"));
  try {
    const file = join(dir, "example.mjs");
    writeFileSync(file, example.replace('"@valancex/mesh-compiler"', JSON.stringify(index)));
    const output = execFileSync(process.execPath, [file], { encoding: "utf8" });
    assert.equal(output.trimEnd(), expected.trimEnd());
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
