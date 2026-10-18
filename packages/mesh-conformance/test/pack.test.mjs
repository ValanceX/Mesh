// The tarball holds the vectors and nothing else: exactly the files of
// `examples/conformance/`, byte for byte, plus the package's own three.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = join(here, "..");
const source = join(packageDir, "..", "..", "examples", "conformance");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

const walk = (dir) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)],
  );
const hash = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

test("the tarball is examples/conformance/ and the package's own files", () => {
  const dir = mkdtempSync(join(tmpdir(), "mesh-conformance-pack-"));
  try {
    const [packed] = JSON.parse(
      execFileSync(npm, ["pack", "--json", "--pack-destination", dir], {
        cwd: packageDir,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        shell: process.platform === "win32",
      }),
    );
    const shipped = packed.files.map((file) => file.path).sort();
    const expected = [
      "LICENSE",
      "README.md",
      "package.json",
      ...walk(source).map((path) => `conformance/${relative(source, path).split("\\").join("/")}`),
    ].sort();
    assert.deepEqual(shipped, expected);

    // And each vector is the repository's, byte for byte.
    execFileSync("tar", ["-xzf", join(dir, packed.filename), "-C", dir]);
    for (const path of walk(source)) {
      const inPackage = join(dir, "package", "conformance", relative(source, path));
      assert.equal(hash(inPackage), hash(path), relative(source, path));
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the package follows MESH's version", () => {
  const cargo = readFileSync(join(packageDir, "..", "..", "Cargo.toml"), "utf8");
  const version = cargo.match(/^version = "([^"]+)"/m)[1];
  const own = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));
  assert.equal(own.version, version);
});
