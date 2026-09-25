#!/usr/bin/env node
/**
 * Starts the native `mesh-lsp` for this platform, and gets out of the
 * way (outline v0.4 D8): the same arguments, the same stdin, stdout and
 * stderr (inherited, never read), and the same exit. It has no LSP logic
 * of its own. It writes only to stderr, and only when it can't start the
 * server.
 *
 * `MESH_LSP_TEST_TARGETS`, a path to a JSON table in `targets.ts`'s
 * shape, replaces the table. It exists for this package's own tests and
 * isn't part of its API.
 */

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { TARGETS, type Target } from "./targets.js";

function fail(message: string): never {
  process.stderr.write(`mesh-lsp: ${message}\n`);
  process.exit(1);
}

function targets(): Record<string, Target> {
  const table = process.env.MESH_LSP_TEST_TARGETS;
  return table ? (JSON.parse(readFileSync(table, "utf8")) as Record<string, Target>) : TARGETS;
}

const key = `${process.platform}-${process.arch}`;
const target = targets()[key];
if (!target) {
  fail(
    `there's no mesh-lsp binary for ${key}; there are for ${Object.keys(TARGETS).join(", ")}. ` +
      "Anywhere Rust runs, `cargo install --path crates/mesh-lsp` in a clone of " +
      "https://github.com/ValanceX/Mesh builds it.",
  );
}

let binary: string;
try {
  const manifest = createRequire(import.meta.url).resolve(`${target.package}/package.json`);
  binary = join(dirname(manifest), "bin", target.binary);
} catch {
  fail(
    `${target.package}, which holds the mesh-lsp binary for ${key}, isn't installed. ` +
      "npm installs it with @valancex/mesh-lsp unless optional dependencies are omitted: " +
      `reinstall without --omit=optional, or install ${target.package} too.`,
  );
}

const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });

const SIGNALS: NodeJS.Signals[] =
  process.platform === "win32" ? ["SIGINT", "SIGTERM"] : ["SIGINT", "SIGTERM", "SIGHUP"];
const forward = (signal: NodeJS.Signals) => {
  child.kill(signal);
};
for (const signal of SIGNALS) {
  process.on(signal, forward);
}

child.on("error", (error) => {
  fail(`couldn't start ${binary}: ${error.message}`);
});

child.on("exit", (code, signal) => {
  for (const each of SIGNALS) {
    process.off(each, forward);
  }
  if (signal) {
    // Die the way the server died, so whoever started us sees the same.
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
