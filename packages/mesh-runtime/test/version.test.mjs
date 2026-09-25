// The wrapper only drives a runtime module of its own version (I10):
// a module reporting another version, or none, is refused before
// any other export is called, and nothing is kept.
import assert from "node:assert/strict";
import { test } from "node:test";
import { init, MeshVersionError, render } from "../dist/index.js";
import { MODEL, SNAPSHOT, template } from "./common.mjs";

const encoder = new TextEncoder();

/** `value` as unsigned LEB128, WebAssembly's encoding of lengths. */
function leb128(value) {
  const bytes = [];
  do {
    let byte = value & 0x7f;
    value >>>= 7;
    if (value !== 0) {
      byte |= 0x80;
    }
    bytes.push(byte);
  } while (value !== 0);
  return bytes;
}

/** A vector: its length, then its bytes. */
function vec(bytes) {
  return [...leb128(bytes.length), ...bytes];
}

function section(id, bytes) {
  return [id, ...vec(bytes)];
}

/** The exports the wrapper checks for before a call. */
const RENDER_EXPORTS = [
  "mesh_alloc",
  "mesh_free",
  "mesh_render",
  "mesh_dispatch",
  "mesh_result_ptr",
  "mesh_result_len",
  "mesh_result_clear",
];

/**
 * A module exporting `memory`, `mesh_version_ptr`/`mesh_version_len` over
 * a data segment holding `version`, and, with `callExports`, a stub for
 * every export a call needs: all a module needs to pass for a MESH
 * module except the version.
 */
function moduleReporting(version, { callExports = false } = {}) {
  const text = [...encoder.encode(version)];
  const name = (s) => vec([...encoder.encode(s)]);
  const constant = (value) => [0x00, 0x41, value, 0x0b]; // no locals; i32.const value; end
  const stubs = callExports ? RENDER_EXPORTS : [];
  const functions = 2 + stubs.length;
  return new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, [0x01, 0x60, 0x00, 0x01, 0x7f]), // one type: () -> i32
    ...section(3, [functions, ...Array(functions).fill(0x00)]), // functions of that type
    ...section(5, [0x01, 0x00, 0x01]), // one memory, one page
    ...section(7, [
      1 + functions,
      ...name("memory"), 0x02, 0x00,
      ...name("mesh_version_ptr"), 0x00, 0x00,
      ...name("mesh_version_len"), 0x00, 0x01,
      ...stubs.flatMap((stub, index) => [...name(stub), 0x00, 2 + index]),
    ]),
    ...section(10, [
      functions,
      ...vec(constant(16)),
      ...vec(constant(text.length)),
      ...stubs.flatMap(() => vec(constant(0))),
    ]),
    ...section(11, [0x01, 0x00, 0x41, 16, 0x0b, ...vec(text)]),
  ]);
}

test("a module of another version is refused", async () => {
  // Everything but the version is right, so only the comparison refuses it.
  const error = await init(moduleReporting("0.0.0-mismatch", { callExports: true })).then(
    () => assert.fail("init should have failed"),
    (error) => error,
  );
  assert.ok(error instanceof MeshVersionError, String(error));
  assert.match(error.message, /but its WebAssembly module is 0\.0\.0-mismatch/);
});

test("a module with no version is refused", async () => {
  const empty = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
  await assert.rejects(init(empty), MeshVersionError);
});

test("a module of the right version without the render exports is refused", async () => {
  const pkg = JSON.parse(
    await import("node:fs/promises").then(({ readFile }) =>
      readFile(new URL("../package.json", import.meta.url), "utf8"),
    ),
  );
  await assert.rejects(init(moduleReporting(pkg.version)), /has no mesh_alloc/);
});

test("after a refused module, nothing is kept: Node loads the package's own", async () => {
  await assert.rejects(init(moduleReporting("9.9.9")), MeshVersionError);
  const templates = [await template("view", "<text>{name}</text>")];
  const result = await render({ program: { root: "view", templates }, model: MODEL, snapshot: SNAPSHOT });
  assert.ok(result.render);
});
