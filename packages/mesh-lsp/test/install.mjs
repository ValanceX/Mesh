// npm's behaviour with these packages, tested rather than assumed
// (outline v0.4 D8), on one target's own runner with its real binary:
//
// 1. From a registry, npm installs the launcher and exactly this
//    platform's package, and `mesh-lsp` answers LSP.
// 2. The same with `--ignore-scripts`: no package needs a script.
// 3. Offline, from the two tarballs, it installs and answers, and the
//    binary is executable.
// 4. The launcher alone (as on an unsupported platform, where npm skips
//    every platform package) installs, and `mesh-lsp` exits 1 naming the
//    package it needs.
//
// The registry is a local verdaccio, serving only what this script
// publishes to it: the launcher, this platform's package with its real
// binary, and the other four with a placeholder, which npm must skip.
//
//   node test/install.mjs <binary>
//
// Not part of `npm test`: it needs a native binary, and starts a registry.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
  accessSync,
  constants,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { get } from "node:http";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const VERDACCIO = "verdaccio@6.10.4";

const binary = resolve(process.argv[2] ?? "");
assert.ok(existsSync(binary), "usage: node test/install.mjs <binary>");

const here = dirname(fileURLToPath(import.meta.url));
const packages = join(here, "..", "..");
const key = `${process.platform}-${process.arch}`;
const windows = process.platform === "win32";
const npm = windows ? "npm.cmd" : "npm";
const TARGETS = ["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64", "win32-x64"];
assert.ok(TARGETS.includes(key), `${key} isn't a shipped target`);

const work = mkdtempSync(join(tmpdir(), "mesh-lsp-install-"));

function run(command, args, cwd, extra = {}) {
  return execFileSync(command, args, { cwd, encoding: "utf8", shell: windows, ...extra });
}

function project(name) {
  const dir = join(work, name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name, private: true }));
  return dir;
}

/** The installed `@valancex/*` packages in `dir`. */
function installed(dir) {
  return readdirSync(join(dir, "node_modules", "@valancex")).sort();
}

/** Speaks LSP to `command`: initialize, shutdown, exit. Resolves with its exit code and stdout. */
function roundTrip(command, args = []) {
  const frame = (message) => {
    const body = JSON.stringify(message);
    return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
  };
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { shell: windows, stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (data) => (stdout += data));
    child.stderr.on("data", (data) => (stderr += data));
    child.on("error", reject);
    child.on("exit", (code) => resolve({ code, stdout, stderr }));
    child.stdin.write(
      frame({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: null, rootUri: null, capabilities: {} } }) +
        frame({ jsonrpc: "2.0", method: "initialized", params: {} }) +
        frame({ jsonrpc: "2.0", id: 2, method: "shutdown" }) +
        frame({ jsonrpc: "2.0", method: "exit" }),
    );
    child.stdin.end();
  });
}

async function assertAnswers(shim, what) {
  const { code, stdout, stderr } = await roundTrip(shim);
  assert.equal(code, 0, `${what}: exit ${code}: ${stderr}`);
  assert.match(stdout, /"capabilities"/, `${what}: no initialize result`);
}

function shimOf(dir) {
  return join(dir, "node_modules", ".bin", windows ? "mesh-lsp.cmd" : "mesh-lsp");
}

async function freePort() {
  return new Promise((resolve) => {
    const server = createServer();
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

/** The five platform packages: this one with its binary, the others with a placeholder. */
function stagePlatformPackages() {
  const staged = [];
  for (const target of TARGETS) {
    const dir = join(work, "staged", `mesh-lsp-${target}`);
    cpSync(join(packages, `mesh-lsp-${target}`), dir, {
      recursive: true,
      filter: (source) => !source.includes(`${join(`mesh-lsp-${target}`, "bin")}`),
    });
    const exe = target.startsWith("win32") ? "mesh-lsp.exe" : "mesh-lsp";
    mkdirSync(join(dir, "bin"), { recursive: true });
    if (target === key) {
      cpSync(binary, join(dir, "bin", exe));
    } else {
      writeFileSync(join(dir, "bin", exe), "a placeholder: npm must never install this package here\n");
    }
    staged.push(dir);
  }
  return staged;
}

/** Whether `url` answers 200 within a second. */
function answers(url) {
  return new Promise((resolve) => {
    const request = get(url, { timeout: 1000 }, (response) => {
      response.resume();
      resolve(response.statusCode === 200);
    });
    request.on("timeout", () => request.destroy());
    request.on("error", () => resolve(false));
  });
}

async function withRegistry(body) {
  const port = await freePort();
  const registry = `http://127.0.0.1:${port}/`;
  const storage = join(work, "registry");
  const config = join(work, "verdaccio.yaml");
  writeFileSync(
    config,
    [
      `storage: ${JSON.stringify(storage)}`,
      "auth:",
      "  htpasswd:",
      `    file: ${JSON.stringify(join(work, "htpasswd"))}`,
      "uplinks: {}",
      "packages:",
      "  '@valancex/*':",
      "    access: $all",
      "    publish: $all",
      "  '**':",
      "    access: $all",
      "log: { type: stdout, format: pretty, level: error }",
      "",
    ].join("\n"),
  );
  // Installed here and started with this `node`, so the server is our
  // own child and stops with it (`npx` would leave it running).
  const tools = join(work, "tools");
  mkdirSync(tools, { recursive: true });
  run(npm, ["install", "--no-audit", "--no-fund", "--prefix", tools, VERDACCIO], work, { stdio: "ignore" });
  const main = join(tools, "node_modules", "verdaccio", "bin", "verdaccio");
  const server = spawn(process.execPath, [main, "--config", config, "--listen", `127.0.0.1:${port}`], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  try {
    for (let attempt = 0; !(await answers(`${registry}-/ping`)); attempt++) {
      assert.ok(attempt < 300, "verdaccio didn't start");
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    // Any token will do: the config lets anyone publish @valancex/*.
    const npmrc = join(work, ".npmrc");
    writeFileSync(npmrc, `registry=${registry}\n//127.0.0.1:${port}/:_authToken=local-test\n`);
    await body({ registry, npmrc });
  } finally {
    server.kill();
  }
}

try {
  // 1 and 2: from a registry.
  await withRegistry(async ({ npmrc }) => {
    const env = { ...process.env, npm_config_userconfig: npmrc };
    for (const dir of [...stagePlatformPackages(), join(packages, "mesh-lsp")]) {
      run(npm, ["publish", "--no-provenance"], dir, { env, stdio: "ignore" });
    }
    for (const flags of [[], ["--ignore-scripts"]]) {
      const dir = project(`registry${flags.join("")}`);
      run(npm, ["install", "--no-audit", "--no-fund", ...flags, "@valancex/mesh-lsp"], dir, { env, stdio: "ignore" });
      assert.deepEqual(installed(dir), ["mesh-lsp", `mesh-lsp-${key}`], `with ${flags}`);
      await assertAnswers(shimOf(dir), `from the registry ${flags}`);
      console.log(`ok: from a registry ${flags.join(" ")}: exactly @valancex/mesh-lsp-${key}`);
    }
  });

  // 3 and 4: offline, from tarballs.
  const tarballs = join(work, "tarballs");
  mkdirSync(tarballs, { recursive: true });
  const pack = (dir) =>
    join(tarballs, JSON.parse(run(npm, ["pack", "--json", "--pack-destination", tarballs], dir))[0].filename);
  const launcher = pack(join(packages, "mesh-lsp"));
  const platform = pack(join(work, "staged", `mesh-lsp-${key}`));

  const offline = project("offline");
  run(npm, ["install", "--offline", "--no-audit", "--no-fund", launcher, platform], offline, { stdio: "ignore" });
  const exe = windows ? "mesh-lsp.exe" : "mesh-lsp";
  const installedBinary = join(offline, "node_modules", "@valancex", `mesh-lsp-${key}`, "bin", exe);
  if (!windows) {
    accessSync(installedBinary, constants.X_OK);
  }
  await assertAnswers(shimOf(offline), "offline, from tarballs");
  console.log("ok: offline, from tarballs, and the binary is executable");

  const alone = project("unsupported");
  run(npm, ["install", "--offline", "--no-audit", "--no-fund", launcher], alone, { stdio: "ignore" });
  const { code, stderr } = await roundTrip(shimOf(alone));
  assert.equal(code, 1);
  assert.match(stderr, new RegExp(`@valancex/mesh-lsp-${key}.*isn't installed`));
  console.log("ok: without a platform package, mesh-lsp exits 1 and names it");
} finally {
  rmSync(work, { recursive: true, force: true });
}
