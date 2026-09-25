// The review test (I11): `src/` encodes and transports, and evaluates,
// converts, coerces, normalizes, substitutes for absence, decides
// composite or primitive, resolves a handler identifier, and judges a
// value nowhere. This finds the ways that would show in the source; a
// reviewer reads the rest (recorded in Pass 3's "As built").
//
// It reads the code with comments removed. Each rule lists its
// exceptions, each with its reason.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { packageDir } from "./common.mjs";

const SRC = join(packageDir, "src");

/** A source file's code: comments removed, strings kept. */
function code(text) {
  let out = "";
  for (let i = 0; i < text.length; ) {
    const two = text.slice(i, i + 2);
    if (two === "//") {
      while (i < text.length && text[i] !== "\n") i++;
    } else if (two === "/*") {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 2;
    } else if (text[i] === '"' || text[i] === "'" || text[i] === "`") {
      const quote = text[i];
      let j = i + 1;
      while (j < text.length && text[j] !== quote) j += text[j] === "\\" ? 2 : 1;
      out += text.slice(i, j + 1);
      i = j + 1;
    } else {
      out += text[i++];
    }
  }
  return out;
}

const files = readdirSync(SRC)
  .filter((name) => name.endsWith(".ts"))
  .sort()
  .map((name) => ({ name, code: code(readFileSync(join(SRC, name), "utf8")) }));

/** Every occurrence of `pattern` in `src/`, as `file: line`. */
function find(pattern) {
  const found = [];
  for (const { name, code: text } of files) {
    text.split("\n").forEach((line, index) => {
      if (pattern.test(line)) found.push(`${name}:${index + 1}: ${line.trim()}`);
    });
  }
  return found;
}

/** `found`, less the lines an exception covers. */
function unexcused(found, exceptions) {
  return found.filter((line) => !exceptions.some(({ file, text }) => line.startsWith(`${file}:`) && line.includes(text)));
}

test("nothing converts a value to text or a number, or compares values", () => {
  const pattern = /\bString\(|\bNumber\(|\.toString\(|JSON\.stringify|parseFloat|parseInt|Object\.is\b|\.toFixed\(|\.toPrecision\(|\.toExponential\(|\bIntl\b|localeCompare|isNaN|isFinite/;
  assert.deepEqual(unexcused(find(pattern), []), []);
});

test("nothing reads the tree or an intent after parsing them", () => {
  // The result document's envelope (`tree`, `intent`, `diagnostics`) is
  // read to return its parts; nothing inside them is.
  const pattern = /\.(root|children|props|events|arguments|command|key|component|text|location|code)\b/;
  const exceptions = [
    { file: "engine.ts", text: "input.program.root", reason: "the host's own program input, passed to the module" },
    { file: "engine.ts", text: "program.root", reason: "the host's own program input, checked to be a string" },
    { file: "engine.ts", text: "render.#root", reason: "the render's kept root name, passed back to the module" },
  ];
  assert.deepEqual(unexcused(find(pattern), exceptions), []);
});

test("nothing names a MESH type", () => {
  // A MESH type kind as a string would mean the package was deciding
  // something about types. `typeof` results are JavaScript's kinds,
  // which the encoder writes as tags.
  const pattern = /["'`](any|optional|record|list|named|null)["'`]/;
  assert.deepEqual(unexcused(find(pattern), []), []);
  const typeofs = find(/["'`](string|number|boolean)["'`]/);
  const exceptions = [
    { file: "encode.ts", text: "case \"", reason: "the encoder's switch on `typeof`, to write each JavaScript kind's tag" },
    { file: "encode.ts", text: "typeof name === \"string\"", reason: "an unsupported object's constructor name, reported as a fact of its kind" },
    { file: "engine.ts", text: "typeof", reason: "checking the host's arguments are strings (JavaScript hygiene, not MESH validation)" },
  ];
  assert.deepEqual(unexcused(typeofs, exceptions), []);
});

test("nothing substitutes for absence, or normalizes a number", () => {
  // `?? ` or `|| ` on a value would substitute a default; -0, NaN and
  // the infinities pass through as their bits.
  const pattern = /\?\?|\|\| *["'`0-9[{]|Math\.|-0\b|\bNaN\b|Infinity/;
  const exceptions = [
    { file: "engine.ts", text: "process.versions?.node", reason: "detecting Node, not a value" },
  ];
  assert.deepEqual(unexcused(find(pattern), exceptions), []);
});

test("the only code that walks host values is the encoder", () => {
  const walkers = find(/Object\.keys|Array\.isArray|getPrototypeOf|\bin object\b/);
  const exceptions = [
    { file: "encode.ts", text: "", reason: "the encoder: the one walk of host values (§9.8.6)" },
    { file: "engine.ts", text: "Array.isArray(program.templates)", reason: "the host's template list, checked to be an array" },
    { file: "engine.ts", text: "Array.isArray((value as", reason: "the module's result envelope has a diagnostics array" },
    { file: "engine.ts", text: "Array.isArray(value)) {", reason: "the module's result document is an object" },
  ];
  assert.deepEqual(unexcused(walkers, exceptions), []);
});
