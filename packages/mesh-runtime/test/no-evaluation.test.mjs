// The review test (I11): `src/` encodes and transports, and evaluates,
// converts, coerces, normalizes, substitutes for absence, decides
// composite or primitive, resolves a handler identifier, and judges a
// value nowhere. This finds the ways that would show in the source; a
// reviewer reads the rest (recorded in Pass 3's "As built").
//
// It reads the code with comments removed. Each rule lists its
// exceptions, each with its reason.
//
// One file is excused from three rules, for one reason: `patches.ts` applies
// the runtime's patches to the previous tree (what a renderer does with a
// patch list), so that `update` returns what changed and not the whole tree.
// It moves parts by key. It reads no value, and converts, defaults and judges
// none: the test below that names it also checks it makes no text or number.
//
// A second file reads host values: `changes.ts`, the host-side `diff` from one
// snapshot to the next. It compares two values for sameness as the boundary
// does and carries the host's own values into edits, for the runtime to judge.
// It is on no path of render, update or dispatch (the engine doesn't import
// it): the tests below check that, and that it makes no text and no number.
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
  const exceptions = [
    { file: "changes.ts", text: "Object.is(", reason: "diff compares two host values for sameness, as the boundary does (a number by its bits)" },
    { file: "changes.ts", text: "Object.keys(", reason: "diff lists a record's present fields, to compare two records" },
  ];
  assert.deepEqual(unexcused(find(pattern), exceptions), []);
});

test("nothing reads the tree or an intent after parsing them", () => {
  // The result document's envelope (`tree`, `intent`, `events`, `diagnostics`)
  // is read to return its parts; nothing inside them is.
  const pattern = /\.(root|children|props|events|arguments|command|key|component|text|location|code)\b/;
  const exceptions = [
    { file: "engine.ts", text: "input.program.root", reason: "the host's own program input, passed to the module" },
    { file: "engine.ts", text: "program.root", reason: "the host's own program input, checked to be a string" },
    { file: "engine.ts", text: "render.#root", reason: "the render's kept root name, passed back to the module" },
    { file: "patches.ts", text: "", reason: "applies the runtime's patches to a tree by key, moving parts and reading no value" },
    { file: "changes.ts", text: "", reason: "diff reads two host snapshots (not a tree or an intent) to compute edits; see the file's own test" },
    { file: "engine.ts", text: "this.code = code", reason: "the package's own error stores its own stable code" },
    { file: "engine.ts", text: "result.events", reason: "the result document's envelope key, like `tree` and `intent`: the declared events are returned as the module wrote them" },
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
    { file: "patches.ts", text: "parts.get(key) ??", reason: "a part as the patches left it, else the tree's: a map lookup, not a default for a value" },
    { file: "patches.ts", text: "keys === undefined ?", reason: "a node's children's keys, if a patch changed them: a map lookup" },
  ];
  assert.deepEqual(unexcused(find(pattern), exceptions), []);
});

test("the only code that walks host values is the encoder", () => {
  const walkers = find(/Object\.keys|Array\.isArray|getPrototypeOf|\bin object\b/);
  const exceptions = [
    { file: "encode.ts", text: "", reason: "the encoder: the one walk of host values (§9.8.6)" },
    { file: "patches.ts", text: "", reason: "applies patches by key: counts a record's entries to drop an empty `propText`, as the runtime's tree has none; reads no value" },
    { file: "changes.ts", text: "", reason: "diff walks host values to compare them, as the encoder does to encode them; the only other place" },
    { file: "engine.ts", text: "Array.isArray(program.templates)", reason: "the host's template list, checked to be an array" },
    { file: "engine.ts", text: "Array.isArray((value as", reason: "the module's result envelope has a diagnostics array" },
    { file: "engine.ts", text: "Array.isArray(result.events)", reason: "the module's result envelope has an events array" },
    { file: "engine.ts", text: "Array.isArray(value)) {", reason: "the module's result document is an object" },
  ];
  assert.deepEqual(unexcused(walkers, exceptions), []);
});

test("the one file that reads a tree, patches.ts, makes no text and no number, and judges nothing", () => {
  const { code: text } = files.find(({ name }) => name === "patches.ts");
  // No conversion, comparison of values, formatting, or defaulting a value.
  assert.deepEqual(
    text.split("\n").filter((line) => /\bString\(|\bNumber\(|\.toString\(|JSON\.|parseFloat|parseInt|Object\.is\b|\.toFixed\(|\bIntl\b|isNaN|isFinite|Math\.|\bNaN\b|Infinity/.test(line)),
    [],
  );
  // And it never looks inside a prop's value: `.value` appears only to carry a patch's value into the tree.
  assert.deepEqual(text.split("\n").filter((line) => /\.value\.|\.value\[/.test(line)), []);
});

test("diff, the one other file that reads host values, is on no path of render, update or dispatch, and makes no text and no number", () => {
  // The engine never imports it, so nothing the runtime does depends on it.
  const engine = files.find(({ name }) => name === "engine.ts").code;
  assert.ok(!/from ["']\.\/changes\.js["']/.test(engine), "engine.ts must not import changes.ts");
  const { code: text } = files.find(({ name }) => name === "changes.ts");
  assert.deepEqual(
    text.split("\n").filter((line) => /\bString\(|\bNumber\(|\.toString\(|JSON\.|parseFloat|parseInt|\.toFixed\(|\bIntl\b|isNaN|isFinite|Math\.|\bNaN\b|Infinity|\?\? /.test(line)),
    [],
  );
});
