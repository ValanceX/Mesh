// The MPRX TextMate grammar, checked with the engine VS Code itself uses (vscode-textmate with the Oniguruma regex engine), against this repository's own MPRX: the Tree-sitter capture
// files and every fixture and example. The grammar colours text and decides nothing else; the compiler decides what is valid.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, "..", "..", "..");
const require = createRequire(import.meta.url);
const { INITIAL, Registry, parseRawGrammar } = require("vscode-textmate");
const { loadWASM, OnigScanner, OnigString } = require("vscode-oniguruma");

await loadWASM(readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer);

const grammar = await new Registry({
  onigLib: Promise.resolve({ createOnigScanner: (patterns) => new OnigScanner(patterns), createOnigString: (text) => new OnigString(text) }),
  loadGrammar: async () => parseRawGrammar(readFileSync(join(here, "..", "syntaxes", "mprx.tmLanguage.json"), "utf8"), "mprx.tmLanguage.json"),
}).loadGrammar("source.mprx");

/** Every token of `text` with its scopes below `source.mprx`, and whether the grammar ended at the top level. */
const tokenize = (text) => {
  let state = INITIAL;
  const tokens = [];

  for (const line of text.split("\n")) {
    const result = grammar.tokenizeLine(line, state);

    for (const token of result.tokens) { tokens.push({ text: line.slice(token.startIndex, token.endIndex), scopes: token.scopes.slice(1) }); }

    state = result.ruleStack;
  }

  return { tokens, closed: state.depth <= 1 };
};

const scope = (tokens, text, nth = 0) => {
  const found = tokens.filter((token) => token.text === text)[nth];

  assert.ok(found, `no token "${text}" in ${JSON.stringify(tokens.map((token) => token.text))}`);

  return found.scopes.at(-1);
};

test("tags, attributes, strings and delimiters", () => {
  const { tokens, closed } = tokenize('<user-card compact="yes" label={name} />');

  assert.equal(scope(tokens, "user-card"), "entity.name.tag.mprx");
  assert.equal(scope(tokens, "<"), "punctuation.definition.tag.begin.mprx");
  assert.equal(scope(tokens, "/>"), "punctuation.definition.tag.end.mprx");
  assert.equal(scope(tokens, "compact"), "entity.other.attribute-name.mprx");
  assert.equal(scope(tokens, "yes"), "string.quoted.double.mprx");
  assert.equal(scope(tokens, "label"), "entity.other.attribute-name.mprx");
  assert.ok(closed);
});

test("expressions: references, members, literals, operators, commands and the event value", () => {
  const { tokens, closed } = tokenize('<a x={user.name} y={count + 1 > 2 ? "big" : null} on.click={save($event, true)} />');

  assert.equal(scope(tokens, "user"), "variable.other.readwrite.mprx");
  assert.equal(scope(tokens, "name"), "variable.other.property.mprx");
  assert.equal(scope(tokens, "1"), "constant.numeric.mprx");
  assert.equal(scope(tokens, ">"), "keyword.operator.mprx");
  assert.equal(scope(tokens, "?"), "keyword.operator.mprx");
  assert.equal(scope(tokens, "big"), "string.quoted.double.mprx");
  assert.equal(scope(tokens, "null"), "constant.language.null.mprx");
  assert.equal(scope(tokens, "on"), "keyword.other.event.mprx");
  assert.equal(scope(tokens, "click"), "entity.other.attribute-name.event.mprx");
  assert.equal(scope(tokens, "save"), "entity.name.function.mprx");
  assert.equal(scope(tokens, "$event"), "variable.language.event.mprx");
  assert.equal(scope(tokens, "true"), "constant.language.boolean.mprx");
  assert.ok(closed);
});

test("text interpolation, closing tags, an object argument; plain text has no scope", () => {
  const { tokens, closed } = tokenize('<button on.click={save(user, { force: true })}>Save {user.name}</button>');

  assert.equal(scope(tokens, "force"), "variable.other.property.mprx");
  assert.equal(scope(tokens, "name"), "variable.other.property.mprx");
  assert.deepEqual(tokens.find((token) => token.text === "Save ").scopes, []);
  assert.deepEqual(tokens.filter((token) => token.text === "button").map((token) => token.scopes.at(-1)), ["entity.name.tag.mprx", "entity.name.tag.mprx"]);
  assert.ok(closed);
});

test("across lines, and a string with escaped quotes", () => {
  const { tokens, closed } = tokenize('<page\n  title="say \\"hi\\""\n  on.close={shut()}\n>\n  text\n</page>');

  assert.deepEqual(tokens.filter((token) => token.scopes.at(-1) === "constant.character.escape.mprx").map((token) => token.text), ['\\"', '\\"']);
  assert.equal(scope(tokens, "shut"), "entity.name.function.mprx");
  assert.ok(closed);
});

const mprxIn = (dir) => readdirSync(dir, { recursive: true, encoding: "utf8" }).filter((file) => file.endsWith(".mprx")).map((file) => join(dir, file));

test("every valid MPRX in the repository returns to the top level and has no invalid scope", () => {
  const files = [...mprxIn(join(repo, "examples")), ...mprxIn(join(repo, "docs"))].filter((file) => !/[\\/]fail[\\/]|broken/.test(file));

  // A guard against a moved directory making this pass on nothing.
  assert.ok(files.length >= 25, `only ${files.length} files found`);

  for (const file of files) {
    const { tokens, closed } = tokenize(readFileSync(file, "utf8").trimEnd());

    assert.ok(closed, `${file} does not return to the top level`);
    assert.equal(tokens.filter((token) => token.scopes.some((each) => each.startsWith("invalid"))).length, 0, file);
  }
});

test("a file that does not parse is still tokenized, never rejected (the compiler reports the error)", () => {
  const files = [...mprxIn(join(repo, "examples")).filter((file) => /[\\/]fail[\\/]/.test(file)), join(repo, "grammar", "tree-sitter-mprx", "test", "captures", "broken.mprx")];

  assert.ok(files.length >= 2, "no failing fixtures found");

  for (const file of files) {
    assert.doesNotThrow(() => tokenize(readFileSync(file, "utf8")), file);
  }
});
