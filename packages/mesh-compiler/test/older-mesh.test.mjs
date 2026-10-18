// The compatibility promise of a new type kind: a MESH from before it
// reads a manifest that uses it as `manifest-unknown-kind`, and nothing
// else. This installs the last release without `enum` (0.13.0) from the
// registry, so it needs the network.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import * as current from "../dist/index.js";

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

const examples = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "examples");

/** The last release before 0.14, installed from the registry into a temporary directory. */
async function installed(run) {
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
    return await run(await import(pathToFileURL(join(dir, "package", "dist", "index.js")).href));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

const codes = (result) => result.diagnostics.diagnostics.map((d) => d.code);

test("a MESH from before 0.14 reports an enum as manifest-unknown-kind", async () => {
  await installed(async (older) => {
    const result = await older.compile({
      source: "<text>{tone}</text>",
      path: "t.mprx",
      model: { manifest, path: "components.json", component: "template" },
    });
    assert.deepEqual(codes(result), ["manifest-unknown-kind"]);
  });
});

test("a MESH from before 0.14 reports an optional snippet as manifest-unknown-kind, and the 0.14 reads it", async () => {
  const optional = JSON.stringify({
    version: 1,
    types: {},
    components: {
      template: {
        props: {},
        events: {},
        commands: {},
        scope: { footer: { kind: "optional", type: { kind: "snippet", params: [] } } },
      },
    },
  });
  const input = {
    source: "<mesh-fragment>x</mesh-fragment>",
    path: "t.mprx",
    model: { manifest: optional, path: "components.json", component: "template" },
  };
  await installed(async (older) => {
    assert.deepEqual(codes(await older.compile(input)), ["manifest-unknown-kind"]);
  });
  assert.deepEqual(codes(await current.compile(input)), []);
});

test("a program that uses nothing new compiles to the same template, fingerprint included, as 0.13", async () => {
  const model = readFileSync(join(examples, "components.json"), "utf8");
  const input = {
    source: readFileSync(join(examples, "users-page.mprx"), "utf8"),
    path: "users-page.mprx",
    model: { manifest: model, path: "components.json", component: "users-page" },
  };
  const now = await current.compile(input);
  assert.deepEqual(codes(now), []);
  await installed(async (older) => {
    const before = await older.compile(input);
    assert.deepEqual(codes(before), []);
    const strip = (template) => ({ ...template, compiler: undefined });
    assert.equal(now.template.fingerprint, before.template.fingerprint);
    assert.deepEqual(strip(now.template), strip(before.template));
  });
});
