// Shared by the package's tests: the programs the Rust runtime's tests
// use, compiled by the real compiler, the schemas, and the native
// harness to compare with.

import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import { compile } from "../../mesh-compiler/dist/index.js";

export const here = dirname(fileURLToPath(import.meta.url));
export const packageDir = join(here, "..");
export const root = join(here, "..", "..", "..");
export const programsDir = join(root, "crates", "mesh-runtime", "tests", "programs");

/** The shipped module, and the test-only one (`test-hooks`). */
export const shippedModule = join(packageDir, "dist", "mesh-runtime.wasm");
export const testModule = join(here, "build", "mesh-runtime-test.wasm");

/** The model and ordinary snapshot the Rust runtime's tests use. */
export const MODEL = readFileSync(join(programsDir, "model.json"), "utf8");
export const SNAPSHOT = JSON.parse(readFileSync(join(programsDir, "snapshot.json"), "utf8"));

/** The template of `component` compiled from `source` against `model` (MODEL by default). */
export async function template(component, source, model = MODEL) {
  const result = await compile({
    source,
    path: `${component}.mprx`,
    model: { manifest: model, path: "model.json", component },
  });
  if (!result.template) {
    throw new Error(`${component} doesn't compile: ${JSON.stringify(result.diagnostics)}`);
  }
  return JSON.stringify(result.template);
}

/**
 * The programs under `crates/mesh-runtime/tests/programs/`: each a
 * directory of `<component>.mprx` templates (root `view`), snapshots,
 * and expected output.
 */
export function programDirs() {
  return readdirSync(programsDir)
    .map((name) => join(programsDir, name))
    .filter((path) => statSync(path).isDirectory())
    .sort();
}

/** A program directory's templates, compiled, in file-name order. */
export async function programTemplates(dir) {
  const sources = readdirSync(dir).filter((name) => name.endsWith(".mprx")).sort();
  return Promise.all(
    sources.map((name) => template(basename(name, ".mprx"), readFileSync(join(dir, name), "utf8"))),
  );
}

function schema(name) {
  const text = readFileSync(join(root, "schemas", name), "utf8");
  return new Ajv2020({ strict: false }).compile(JSON.parse(text));
}

/** Validators for the tree and the runtime diagnostics document. */
export const validateTree = schema("render-v1.schema.json");
export const validateDiagnostics = schema("runtime-diagnostics-v1.schema.json");

/** The intent schema: render-v1's `$defs/intent`. */
export const validateIntent = (() => {
  const document = JSON.parse(readFileSync(join(root, "schemas", "render-v1.schema.json"), "utf8"));
  const ajv = new Ajv2020({ strict: false });
  ajv.addSchema(document);
  return ajv.getSchema(`${document.$id}#/$defs/intent`);
})();

/** The native harness: `MESH_RUNTIME_HARNESS`, or the workspace's debug build. */
export function harnessPath() {
  if (process.env.MESH_RUNTIME_HARNESS) {
    return process.env.MESH_RUNTIME_HARNESS;
  }
  const exe = process.platform === "win32" ? "harness.exe" : "harness";
  const path = join(root, "target", "debug", "examples", exe);
  if (!existsSync(path)) {
    throw new Error(
      `no native harness at ${path}: run \`cargo build -p mesh-runtime-wasm --example harness\`, or set MESH_RUNTIME_HARNESS`,
    );
  }
  return path;
}

/** A running harness: `ask(request)` resolves to its result line, parsed. */
export function startHarness() {
  const child = spawn(harnessPath(), [], { stdio: ["pipe", "pipe", "inherit"] });
  const lines = createInterface({ input: child.stdout });
  const waiting = [];
  lines.on("line", (line) => waiting.shift()(JSON.parse(line)));
  return {
    ask(request) {
      return new Promise((resolve) => {
        waiting.push(resolve);
        child.stdin.write(`${JSON.stringify(request)}\n`);
      });
    },
    close() {
      child.stdin.end();
      return new Promise((resolve) => child.on("close", resolve));
    },
  };
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
