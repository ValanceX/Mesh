// The compatibility promise of a new type kind: a MESH from before it
// reads a manifest that uses it as `manifest-unknown-kind`, and nothing
// else. This installs the last release without `enum` (0.13.0) from the
// registry, so it needs the network.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const manifest = JSON.stringify({
  version: 1,
  types: {},
  components: {
    template: {
      props: {},
      events: {},
      commands: {},
      scope: { tone: { kind: "enum", values: ["info", "danger"] } },
    },
  },
});

test("a MESH from before 0.14 reports an enum as manifest-unknown-kind", async () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-older-"));
  try {
    const [packed] = JSON.parse(
      execFileSync(npm, ["pack", "@valancex/mesh-compiler@0.13.0", "--json", "--pack-destination", dir], {
        cwd: dir,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        shell: process.platform === "win32",
      }),
    );
    execFileSync("tar", ["-xzf", join(dir, packed.filename), "-C", dir]);
    const older = await import(pathToFileURL(join(dir, "package", "dist", "index.js")).href);
    const result = await older.compile({
      source: "<text>{tone}</text>",
      path: "t.mprx",
      model: { manifest, path: "components.json", component: "template" },
    });
    assert.deepEqual(
      result.diagnostics.diagnostics.map((d) => d.code),
      ["manifest-unknown-kind"],
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
