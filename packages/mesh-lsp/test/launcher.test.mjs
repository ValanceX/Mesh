// The launcher is transparent (outline v0.4 D8): arguments, stdin and
// stdout pass through byte for byte, exit codes and signals pass
// through, and when there's nothing to start it says so and exits 1.
//
// The "binary" here is Node itself, hard-linked into a fake platform
// package; the launcher passes its arguments on, so the first is a
// script for that Node to run.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, cpSync, linkSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const key = `${process.platform}-${process.arch}`;
const exe = process.platform === "win32" ? "fake.exe" : "fake";

const SCRIPT = `
const [mode, ...rest] = process.argv.slice(2);
if (mode === "echo") {
  process.stdout.write(JSON.stringify(rest) + "\\n");
  process.stdin.pipe(process.stdout);
} else if (mode === "exit") {
  process.exit(Number(rest[0]));
} else if (mode === "wait") {
  process.stdout.write("ready\\n");
  setInterval(() => {}, 1000);
}
`;

let dir;
let cli;
let script;
let tables;

before(() => {
  dir = mkdtempSync(join(tmpdir(), "mesh-lsp-launcher-"));
  const launcher = join(dir, "node_modules", "@valancex", "mesh-lsp");
  mkdirSync(launcher, { recursive: true });
  copyFileSync(join(here, "..", "package.json"), join(launcher, "package.json"));
  cpSync(join(here, "..", "dist"), join(launcher, "dist"), { recursive: true });
  cli = join(launcher, "dist", "cli.js");

  const fake = join(dir, "node_modules", "@valancex", "fake-mesh-lsp");
  mkdirSync(join(fake, "bin"), { recursive: true });
  writeFileSync(join(fake, "package.json"), JSON.stringify({ name: "@valancex/fake-mesh-lsp", version: "0.0.0" }));
  try {
    linkSync(process.execPath, join(fake, "bin", exe));
  } catch {
    copyFileSync(process.execPath, join(fake, "bin", exe));
  }
  script = join(dir, "fake.mjs");
  writeFileSync(script, SCRIPT);

  tables = {
    fake: join(dir, "fake.json"),
    none: join(dir, "none.json"),
    missing: join(dir, "missing.json"),
  };
  writeFileSync(tables.fake, JSON.stringify({ [key]: { package: "@valancex/fake-mesh-lsp", binary: exe } }));
  writeFileSync(tables.none, JSON.stringify({}));
  writeFileSync(tables.missing, JSON.stringify({ [key]: { package: "@valancex/mesh-lsp-not-installed", binary: exe } }));
});

after(() => rmSync(dir, { recursive: true, force: true }));

function launch(table, args, options = {}) {
  return spawnSync(process.execPath, [cli, ...args], {
    env: { ...process.env, MESH_LSP_TEST_TARGETS: table },
    ...options,
  });
}

test("arguments and streams pass through byte for byte", () => {
  const args = ["plain", "a b", "é😀", "--flag=1", ""];
  const input = Buffer.from("Content-Length: 2\r\n\r\n{}\u0000\xff", "latin1");
  const result = launch(tables.fake, [script, "echo", ...args], { input });
  assert.equal(result.status, 0, result.stderr.toString());
  const expected = Buffer.concat([Buffer.from(JSON.stringify(args) + "\n"), input]);
  assert.deepEqual(result.stdout, expected);
  assert.equal(result.stderr.length, 0);
});

test("exit codes pass through", () => {
  for (const code of [0, 1, 3]) {
    assert.equal(launch(tables.fake, [script, "exit", String(code)]).status, code);
  }
});

// Windows has no POSIX signals to forward.
test("a signal to the launcher reaches the server, and ends both alike", { skip: process.platform === "win32" }, async () => {
  const child = spawn(process.execPath, [cli, script, "wait"], {
    env: { ...process.env, MESH_LSP_TEST_TARGETS: tables.fake },
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((resolve) => child.stdout.once("data", resolve));
  child.kill("SIGTERM");
  const [code, signal] = await new Promise((resolve) => child.on("exit", (...args) => resolve(args)));
  assert.deepEqual([code, signal], [null, "SIGTERM"]);
});

test("an unsupported platform says so and exits 1", () => {
  const result = launch(tables.none, []);
  assert.equal(result.status, 1);
  assert.equal(result.stdout.length, 0);
  assert.match(result.stderr.toString(), /no mesh-lsp binary for .*cargo install --path crates\/mesh-lsp/s);
});

test("a missing platform package is named, and exits 1", () => {
  const result = launch(tables.missing, []);
  assert.equal(result.status, 1);
  assert.equal(result.stdout.length, 0);
  assert.match(result.stderr.toString(), /@valancex\/mesh-lsp-not-installed.*isn't installed/);
});
