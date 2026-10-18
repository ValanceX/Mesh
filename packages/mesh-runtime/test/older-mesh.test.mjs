// The compatibility promise of a new template version: a runtime from before it refuses a template
// that needs it with `assembly-unsupported-format-version` (and runs one that doesn't). This installs
// the last release without `template-v2` (0.13.0) from the registry, so it needs the network.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { compile } from "../../mesh-compiler/dist/index.js";

const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const model = JSON.stringify({
  version: 1,
  types: {},
  components: {
    text: { props: {}, events: {}, commands: {}, scope: {} },
    page: { props: {}, events: {}, commands: {}, scope: {} },
    view: { props: {}, events: {}, commands: {}, scope: { name: { kind: "optional", type: { kind: "string" } } } },
  },
});
const templateOf = async (source) => {
  const result = await compile({ source, path: "view.mprx", model: { manifest: model, path: "m.json", component: "view" } });
  assert.deepEqual(result.diagnostics.diagnostics, []);
  return result.template;
};

test("a runtime from before template-v2 refuses a version 2 template, and runs a version 1 one", async () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-older-"));
  try {
    const [packed] = JSON.parse(
      execFileSync(npm, ["pack", "@valancex/mesh-runtime@0.13.0", "--json", "--pack-destination", dir], {
        cwd: dir,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        shell: process.platform === "win32",
      }),
    );
    execFileSync("tar", ["-xzf", join(dir, packed.filename), "-C", dir]);
    const older = await import(pathToFileURL(join(dir, "package", "dist", "index.js")).href);
    const run = async (template) =>
      older.render({ program: { root: "view", templates: [JSON.stringify(template)] }, model, snapshot: {} });

    const v2 = await templateOf('<page><text>{name ?? "none"}</text></page>');
    assert.equal(v2.version, 2);
    const refused = await run(v2);
    assert.equal(refused.render, undefined);
    assert.deepEqual(
      refused.diagnostics.diagnostics.map((d) => d.code),
      ["assembly-unsupported-format-version"],
    );

    const v1 = await templateOf("<page><text>plain</text></page>");
    assert.equal(v1.version, 1);
    const ran = await run(v1);
    assert.ok(ran.render, JSON.stringify(ran.diagnostics) + " a template that uses nothing of version 2 still runs on the older runtime");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
