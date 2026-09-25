// The runtime module, driven directly at its byte boundary, for the
// parity and number tests: the exact result bytes it keeps, where the
// package's API would parse them.
import { readFileSync } from "node:fs";
import { encodeTexts, encodeValue } from "../../dist/encode.js";

const utf8 = new TextEncoder();
const text = new TextDecoder("utf-8", { fatal: true });

export class Module {
  constructor(path) {
    const module = new WebAssembly.Module(readFileSync(path));
    this.exports = new WebAssembly.Instance(module, {}).exports;
  }

  #call(name, inputs, extra = []) {
    const e = this.exports;
    const buffers = inputs.map((bytes) => {
      const ptr = e.mesh_alloc(bytes.length);
      new Uint8Array(e.memory.buffer, ptr, bytes.length).set(bytes);
      return { ptr, len: bytes.length };
    });
    const status = e[name](...buffers.flatMap(({ ptr, len }) => [ptr, len]), ...extra);
    for (const { ptr, len } of buffers) e.mesh_free(ptr, len);
    const result = text.decode(new Uint8Array(e.memory.buffer, e.mesh_result_ptr(), e.mesh_result_len()));
    e.mesh_result_clear();
    return { status, result };
  }

  /** A harness request's call: the same bytes, through this module. */
  request(request) {
    const bytes = (b64) => new Uint8Array(Buffer.from(b64, "base64"));
    if (request.op === "numbers") {
      return this.#call("mesh_test_number_text", [bytes(request.bits)]);
    }
    const common = [
      utf8.encode(request.root),
      bytes(request.templates),
      utf8.encode(request.model),
      bytes(request.snapshot),
    ];
    if (request.op === "render") {
      return this.#call("mesh_render", common);
    }
    if (request.op === "dispatch") {
      const payload = request.payload === null ? new Uint8Array(0) : bytes(request.payload);
      return this.#call("mesh_dispatch", [...common, utf8.encode(request.handler), payload], [
        request.payload === null ? 0 : 1,
      ]);
    }
    throw new Error(`no operation ${request.op}`);
  }
}

const base64 = (bytes) => Buffer.from(bytes).toString("base64");

/** A render request, encoded exactly as the package encodes it. */
export function renderRequest(root, templates, model, snapshot) {
  return {
    op: "render",
    root,
    templates: base64(encodeTexts(templates)),
    model,
    snapshot: base64(encodeValue(snapshot)),
  };
}

/** A dispatch request after `render`: absent payload when `payload` is undefined. */
export function dispatchRequest(render, handler, payload) {
  return {
    ...render,
    op: "dispatch",
    handler,
    payload: payload === undefined ? null : base64(encodeValue(payload)),
  };
}

/** A number-text request: each number's binary64 bits, little-endian. */
export function numbersRequest(numbers) {
  const bytes = new Uint8Array(numbers.length * 8);
  const view = new DataView(bytes.buffer);
  numbers.forEach((n, i) => view.setFloat64(i * 8, n, true));
  return { op: "numbers", bits: base64(bytes) };
}
