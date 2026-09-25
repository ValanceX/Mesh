// The packed package ships what it should and nothing it shouldn't, and
// works installed from its tarball, with no Rust toolchain and nothing
// else of MESH's.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { MODEL, packageDir, template } from "./common.mjs";

const npm = process.platform === "win32" ? "npm.cmd" : "npm";

const REQUIRED = [
  "package.json",
  "README.md",
  "LICENSE",
  "dist/mesh-runtime.wasm",
  ...["index", "engine", "encode", "types", "version"].flatMap((name) => [
    `dist/${name}.js`,
    `dist/${name}.d.ts`,
  ]),
];

/** Paths that must never ship: sources, tests, build scripts, the test module, maps. */
const FORBIDDEN = [/^src\//, /^test\//, /^scripts\//, /^tsconfig\.json$/, /\.map$/, /test\.wasm$/];

test("the tarball ships the package and installs with no Rust toolchain", async () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-runtime-pack-"));
  try {
    const [packed] = JSON.parse(
      execFileSync(npm, ["pack", "--json", "--pack-destination", dir], {
        cwd: packageDir,
        encoding: "utf8",
        shell: process.platform === "win32",
      }),
    );
    const files = packed.files.map((file) => file.path);
    for (const required of REQUIRED) {
      assert.ok(files.includes(required), `${required} isn't packed: ${files}`);
    }
    for (const file of files) {
      assert.ok(!FORBIDDEN.some((pattern) => pattern.test(file)), `${file} shouldn't be packed`);
    }
    assert.deepEqual(packed.bundled ?? [], []);

    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "project", private: true, type: "module" }));
    execFileSync(
      npm,
      ["install", "--offline", "--no-audit", "--no-fund", join(dir, packed.filename)],
      { cwd: dir, stdio: "ignore", shell: process.platform === "win32" },
    );
    writeFileSync(join(dir, "view.template.json"), await template("view", "<text>Hello, {name}!</text>"));
    writeFileSync(join(dir, "model.json"), MODEL);
    const script = `
      import { readFileSync } from "node:fs";
      import { render, version } from "@valancex/mesh-runtime";
      const result = await render({
        program: { root: "view", templates: [readFileSync("view.template.json", "utf8")] },
        model: readFileSync("model.json", "utf8"),
        snapshot: {
          user: { name: "Ada", active: true }, name: "Ada", count: 1, flag: true, anything: 1,
          users: [], numbers: [], maybeNumbers: [], nothing: null,
        },
      });
      console.log(version, result.render.tree.root.children[0].text);
    `;
    writeFileSync(join(dir, "smoke.mjs"), script);
    // Only Node's own directory on PATH: nothing else, cargo included.
    const output = execFileSync(process.execPath, ["smoke.mjs"], {
      cwd: dir,
      encoding: "utf8",
      env: { PATH: dirname(process.execPath) },
    });
    assert.equal(output.trim(), `${packed.version} Hello, Ada!`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the package has no dependencies (I15)", async () => {
  const pkg = JSON.parse(await import("node:fs/promises").then(({ readFile }) => readFile(join(packageDir, "package.json"), "utf8")));
  for (const field of ["dependencies", "peerDependencies", "optionalDependencies", "bundleDependencies"]) {
    assert.equal(pkg[field], undefined, `${field} must be absent`);
  }
});
