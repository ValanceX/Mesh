/**
 * The value encoding (spec §9.8.6; `mesh_runtime::encoding` documents the
 * bytes): the only code in the package that walks a host's values.
 *
 * It records what it finds and decides nothing (I11): it knows no MESH
 * type, doesn't reject NaN or an unpaired surrogate, and looks at no
 * MESH name. The runtime decodes the bytes and judges them, exactly as it
 * judges a native host's values. Not public API.
 *
 * - An array is read by index up to its `length`, so a hole is seen; a
 *   hole or an `undefined` element is an absent element.
 * - A plain object (its prototype is `Object.prototype` or `null`) is a
 *   record of its own enumerable string-keyed properties
 *   (`Object.keys`), in that order; a property whose value is
 *   `undefined` is left out, which is absence.
 * - Anything else is unsupported, with its kind: `typeof` for a
 *   function, symbol or bigint, and `object`, or `object:` and its
 *   constructor's name, for any other object.
 * - A value that is its own ancestor is unsupported as `cycle`, so
 *   encoding always ends. A value reached twice without a cycle is
 *   encoded twice.
 */

const ABSENT = 0;
const NULL = 1;
const FALSE = 2;
const TRUE = 3;
const NUMBER = 4;
const STRING = 5;
const LIST = 6;
const RECORD = 7;
const UNSUPPORTED = 8;

/** A growable little-endian byte buffer. */
class Writer {
  private bytes = new Uint8Array(256);
  private view = new DataView(this.bytes.buffer);
  length = 0;

  private reserve(n: number): void {
    if (this.length + n <= this.bytes.length) {
      return;
    }
    let size = this.bytes.length * 2;
    while (size < this.length + n) {
      size *= 2;
    }
    const bytes = new Uint8Array(size);
    bytes.set(this.bytes.subarray(0, this.length));
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer);
  }

  u8(value: number): void {
    this.reserve(1);
    this.bytes[this.length] = value;
    this.length += 1;
  }

  u32(value: number): void {
    this.reserve(4);
    this.view.setUint32(this.length, value, true);
    this.length += 4;
  }

  f64(value: number): void {
    this.reserve(8);
    this.view.setFloat64(this.length, value, true);
    this.length += 8;
  }

  /** A string body: its UTF-16 code units, whatever they are. */
  units(text: string): void {
    this.u32(text.length);
    this.reserve(text.length * 2);
    for (let index = 0; index < text.length; index++) {
      this.view.setUint16(this.length, text.charCodeAt(index), true);
      this.length += 2;
    }
  }

  result(): Uint8Array {
    return this.bytes.slice(0, this.length);
  }
}

function isPlainObject(value: object): boolean {
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

/** An unsupported object's kind: `object`, with its constructor's name when it has one. */
function objectKind(value: object): string {
  const name = (value as { constructor?: { name?: unknown } }).constructor?.name;
  return typeof name === "string" && name !== "" ? `object:${name}` : "object";
}

function write(writer: Writer, value: unknown, ancestors: Set<object>): void {
  switch (typeof value) {
    case "undefined":
      writer.u8(ABSENT);
      return;
    case "boolean":
      writer.u8(value ? TRUE : FALSE);
      return;
    case "number":
      writer.u8(NUMBER);
      writer.f64(value);
      return;
    case "string":
      writer.u8(STRING);
      writer.units(value);
      return;
    case "bigint":
    case "symbol":
    case "function":
      writer.u8(UNSUPPORTED);
      writer.units(typeof value);
      return;
  }
  if (value === null) {
    writer.u8(NULL);
    return;
  }
  const object = value as object;
  if (ancestors.has(object)) {
    writer.u8(UNSUPPORTED);
    writer.units("cycle");
    return;
  }
  if (Array.isArray(object)) {
    ancestors.add(object);
    writer.u8(LIST);
    writer.u32(object.length);
    for (let index = 0; index < object.length; index++) {
      if (index in object) {
        write(writer, object[index], ancestors);
      } else {
        writer.u8(ABSENT);
      }
    }
    ancestors.delete(object);
    return;
  }
  if (!isPlainObject(object)) {
    writer.u8(UNSUPPORTED);
    writer.units(objectKind(object));
    return;
  }
  ancestors.add(object);
  const record = object as Record<string, unknown>;
  const fields: [string, unknown][] = [];
  for (const name of Object.keys(record)) {
    const field = record[name];
    if (field !== undefined) {
      fields.push([name, field]);
    }
  }
  writer.u8(RECORD);
  writer.u32(fields.length);
  for (const [name, field] of fields) {
    writer.units(name);
    write(writer, field, ancestors);
  }
  ancestors.delete(object);
}

/**
 * One value, a snapshot or a payload, encoded. A top-level `undefined`
 * is encoded as an unsupported value of kind `undefined`; the caller
 * handles an absent payload before encoding.
 */
export function encodeValue(value: unknown): Uint8Array {
  const writer = new Writer();
  if (value === undefined) {
    writer.u8(UNSUPPORTED);
    writer.units("undefined");
  } else {
    write(writer, value, new Set());
  }
  return writer.result();
}

const utf8 = new TextEncoder();

/**
 * A text list: a u32 count, then each text as a u32 byte length and its
 * UTF-8 bytes. The texts are templates, which are JSON documents.
 */
export function encodeTexts(texts: readonly string[]): Uint8Array {
  const encoded = texts.map((text) => utf8.encode(text));
  const writer = new Writer();
  writer.u32(encoded.length);
  const size = 4 + encoded.reduce((sum, bytes) => sum + 4 + bytes.length, 0);
  const list = new Uint8Array(size);
  list.set(writer.result());
  const view = new DataView(list.buffer);
  let at = 4;
  for (const bytes of encoded) {
    view.setUint32(at, bytes.length, true);
    list.set(bytes, at + 4);
    at += 4 + bytes.length;
  }
  return list;
}
