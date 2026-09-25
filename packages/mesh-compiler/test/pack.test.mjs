// The packed package ships what it should and nothing it shouldn't, and
// works installed from its tarball, with no Rust toolchain (outline
// v0.4's "The packages are what we say they are").
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { packageDir } from "./common.mjs";

const npm = process.platform === "win32" ? "npm.cmd" : "npm";

const REQUIRED = [
  "package.json",
  "README.md",
  "LICENSE",
  "dist/mesh.wasm",
  ...["index", "engine", "version", "document"].flatMap((name) => [
    `dist/${name}.js`,
    `dist/${name}.d.ts`,
  ]),
];

/** Paths that must never ship: sources, tests, build scripts, the test module, maps. */
const FORBIDDEN = [/^src\//, /^test\//, /^scripts\//, /^tsconfig\.json$/, /\.map$/, /mesh-test\.wasm$/];

test("the tarball ships the package and installs with no Rust toolchain", () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-compiler-pack-"));
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

    writeFileSync(join(dir, "package.json"), JSON.stringify({ name: "project", private: true, type: "module" }));
    execFileSync(
      npm,
      ["install", "--offline", "--no-audit", "--no-fund", join(dir, packed.filename)],
      { cwd: dir, stdio: "ignore", shell: process.platform === "win32" },
    );
    const script = `
      import { check, version } from "@valancex/mesh-compiler";
      const document = await check({ source: "<a></b>", path: "a.mprx" });
      console.log(version, document.diagnostics[0].code);
    `;
    writeFileSync(join(dir, "smoke.mjs"), script);
    // Only Node's own directory on PATH: nothing else, cargo included.
    const output = execFileSync(process.execPath, ["smoke.mjs"], {
      cwd: dir,
      encoding: "utf8",
      env: { PATH: dirname(process.execPath) },
    });
    assert.equal(output.trim(), `${packed.version} mismatched-closing-tag`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
