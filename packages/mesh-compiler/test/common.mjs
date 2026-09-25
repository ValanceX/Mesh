// Shared by the package's tests: the corpus `mesh check` runs, the
// native CLI to compare with, and the built package.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const here = dirname(fileURLToPath(import.meta.url));
export const packageDir = join(here, "..");
export const root = join(here, "..", "..", "..");
export const examples = join(root, "examples");

/** The shipped module, and the test-only one (`test-hooks`). */
export const shippedModule = join(packageDir, "dist", "mesh.wasm");
export const testModule = join(here, "build", "mesh-test.wasm");

/** The native `mesh`: `MESH_BIN`, or the workspace's debug build. */
export function meshBin() {
  if (process.env.MESH_BIN) {
    return process.env.MESH_BIN;
  }
  const exe = process.platform === "win32" ? "mesh.exe" : "mesh";
  const path = join(root, "target", "debug", exe);
  if (!existsSync(path)) {
    throw new Error(`no native mesh at ${path}: run \`cargo build -p mesh-cli\`, or set MESH_BIN`);
  }
  return path;
}

export function read(relative) {
  return readFileSync(join(examples, relative), "utf8");
}

/** Every `.<extension>` file directly in `examples/<dir>`, relative to `examples/`. */
function files(dir, extension) {
  const found = readdirSync(join(examples, dir), { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(`.${extension}`))
    .map((entry) => (dir ? `${dir}/${entry.name}` : entry.name))
    .sort();
  if (found.length === 0) {
    throw new Error(`examples/${dir} has no .${extension} files`);
  }
  return found;
}

/**
 * Every run `crates/mesh-cli/tests/fixtures.rs` makes, each component
 * explicit: `{ file, model?: { manifest, component } }`, paths relative to
 * `examples/`.
 */
export function corpus() {
  const runs = [];
  for (const dir of ["", "fixtures/pass", "fixtures/fail"]) {
    runs.push(...files(dir, "mprx").map((file) => ({ file })));
  }
  for (const dir of ["fixtures/check/pass", "fixtures/check/fail"]) {
    runs.push(
      ...files(dir, "mprx").map((file) => ({
        file,
        model: { manifest: "fixtures/check/components.json", component: "template" },
      })),
    );
  }
  for (const file of files("", "mprx")) {
    const component = file === "user-card.mprx" ? "user-card-example" : basename(file, ".mprx");
    runs.push({ file, model: { manifest: "components.json", component } });
  }
  for (const manifest of files("fixtures/manifest/fail", "json")) {
    runs.push({ file: "fixtures/manifest/template.mprx", model: { manifest, component: "template" } });
  }
  if (runs.length < 90) {
    throw new Error(`only ${runs.length} runs`);
  }
  return runs;
}

/** `mesh check --format json` on files in `examples/`, parsed. */
export function cli(file, model, cwd = examples) {
  const args = ["check", "--format", "json"];
  if (model) {
    args.push("--model", model.manifest, "--component", model.component);
  }
  args.push(file);
  const result = spawnSync(meshBin(), args, { cwd, encoding: "utf8" });
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(`mesh check ${args.join(" ")}: exit ${result.status}: ${result.stderr}`);
  }
  return JSON.parse(result.stdout);
}

/** The API's input for a corpus run. */
export function input(run) {
  const value = { source: read(run.file), path: run.file };
  if (run.model) {
    value.model = {
      manifest: read(run.model.manifest),
      path: run.model.manifest,
      component: run.model.component,
    };
  }
  return value;
}

/** A small seeded PRNG (mulberry32), so a failing seed reproduces exactly. */
export function prng(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** What a mutation may insert, like `crates/mesh-cli/tests/fuzz.rs`'s. */
const TOKENS = [
  "<", ">", "/>", "</", "{", "}", "(", ")", "[", "]", "=", "\"", ".", ",", ":", "?",
  "!", "-", "+", "==", "&&", "on.", "on.select", "$event", "user", "name", "avatar",
  "selectUser(", "true", "null", "1.5", " ", "\n", "\r\n", "\t", "é", "日本", "😀",
  "﻿", "<a>", "</a>", "<user-card", "{user.", "{[", "{a: ",
];

/** One mutant of `text`: an insertion, a deletion, or a truncation, at code-point boundaries. */
export function mutate(text, random) {
  const points = Array.from(text);
  const at = () => Math.floor(random() * (points.length + 1));
  const choice = random();
  if (choice < 0.5) {
    const token = TOKENS[Math.floor(random() * TOKENS.length)];
    points.splice(at(), 0, ...Array.from(token));
  } else if (choice < 0.85) {
    const start = at();
    points.splice(start, 1 + Math.floor(random() * 8));
  } else {
    points.length = at();
  }
  return points.join("");
}
