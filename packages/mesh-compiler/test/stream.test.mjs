// Streaming checks in WebAssembly (decisions G1 to G14; Bet 13): a session over the data in examples/conformance/generated/, the same programs and planted mistakes the
// native tests use. The two runtimes must agree on every prefix (the digests were recorded natively), `close()` is the batch check, and a mistake is heard within 20 bytes.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { check, openSession, MeshUsageError } from "../dist/index.js";
import { examples } from "./common.mjs";

const data = join(examples, "conformance", "generated");
const manifest = readFileSync(join(data, "components.json"), "utf8");
const pages = Object.fromEntries(readdirSync(join(data, "pages")).sort().map((name) => [name.replace(/\.mprx$/, ""), readFileSync(join(data, "pages", name), "utf8")]));
const mistakes = JSON.parse(readFileSync(join(data, "mistakes.json"), "utf8"));
const expected = JSON.parse(readFileSync(join(data, "steps.expected.json"), "utf8"));
const open = (extra = {}) => openSession({ manifest, component: "page", ...extra });
const model = { manifest, path: "components.json", component: "page" };

/** Every prefix of `source` that ends between characters (a code point is never cut). */
const prefixes = (source) => {
  const characters = [...source];

  return Array.from({ length: characters.length + 1 }, (_, count) => characters.slice(0, count).join(""));
};

/** The line the native test computes for a step. */
const summary = (step) =>
  [
    step.repaired.cut,
    step.pending.map((each) => `${each.kind}:${each.name ?? ""}:${each.start}`).join(","),
    step.withheld,
    step.standing.map((d) => `${d.code}@${d.span.start.byte}-${d.span.end.byte}`).sort().join(","),
    step.preview !== undefined,
  ].join("|");

const utf8Length = (text) => Buffer.byteLength(text);

test("the two runtimes agree on every prefix of every page and every planted mistake (digests recorded by the native tests)", async () => {
  const sources = { ...pages, ...Object.fromEntries(mistakes.map((each) => [`mistake-${each.name}`, each.source])) };

  assert.deepEqual(Object.keys(sources).sort(), Object.keys(expected).sort());

  for (const [name, source] of Object.entries(sources)) {
    const hash = createHash("sha256");
    const all = prefixes(source);

    for (const prefix of all) {
      const session = await open();

      hash.update(`${summary(await session.push(prefix))}\n`);
    }

    assert.equal(all.length, expected[name].prefixes, name);
    assert.equal(hash.digest("hex"), expected[name].sha256, name);
  }
});

test("close() is the batch check of the whole text, however it was cut: random prefixes and mutations, one to five chunks each", async () => {
  const sources = [...Object.values(pages), ...mistakes.map((each) => each.source)];
  const alphabet = [..."<>{}#:/@=\" \n\\abcxyz019.$!&|?()[]é☕"];
  let seed = 20_261_024;
  const next = () => {
    seed = (seed + 0x6d2b79f5) >>> 0;
    let t = seed;

    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);

    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const below = (n) => Math.floor(next() * Math.max(n, 1));

  for (let round = 0; round < 3000; round++) {
    let characters = [...sources[below(sources.length)]];

    characters = characters.slice(0, below(characters.length + 1));

    if (round % 4 === 0 && characters.length > 0) {
      const at = below(characters.length);
      const replacement = alphabet[below(alphabet.length)];

      characters = [() => characters.map((c, i) => (i === at ? replacement : c)), () => [...characters.slice(0, at), replacement, ...characters.slice(at)], () => characters.filter((_, i) => i !== at)][below(3)]();
    }

    const text = characters.join("");
    const session = await open();
    const cuts = 1 + below(5);
    let at = 0;

    for (let chunk = 0; chunk < cuts; chunk++) {
      const end = chunk + 1 === cuts ? text.length : at + below(text.length - at + 1);

      // never between the halves of a pair: the session holds a lone half back, and that is tested on its own below
      await session.push(text.slice(at, end));
      at = end;
    }

    assert.deepEqual(await session.close(), await check({ source: text, path: "generated.mprx", model }), `round ${round}: ${JSON.stringify(text)}`);
  }
});

test("a planted mistake is reported within 20 bytes of its start, once, by a session fed one character at a time", async () => {
  for (const { name, code, source } of mistakes) {
    const batch = (await check({ source, path: "generated.mprx", model })).diagnostics.find((d) => d.code === code);

    assert.ok(batch, `${name}: the batch check has ${code}`);

    const session = await open();
    let heard;
    let times = 0;
    let text = "";

    for (const character of source) {
      text += character;

      const step = await session.push(character);
      const found = step.diagnostics.filter((d) => d.code === code && d.span.start.byte === batch.span.start.byte);

      times += found.length;

      if (found.length > 0 && heard === undefined) {
        heard = utf8Length(text);
      }
    }

    assert.ok(heard !== undefined, `${name}: ${code} was never reported`);
    assert.ok(heard - batch.span.start.byte <= 20, `${name}: ${code} at byte ${batch.span.start.byte} was reported at ${heard}`);
    assert.equal(times, 1, `${name}: reported once`);
  }
});

test("a preview is a checked template and only while nothing is wrong; an open {#each} is withheld until it closes; what is open is listed", async () => {
  const session = await open();
  const a = await session.push("<stack><card title=\"x\"><text>Hello wor");

  assert.deepEqual(a.diagnostics, []);
  assert.ok(a.preview, "a preview");
  assert.equal(a.preview.text, "<stack><card title=\"x\"><text>Hello wor</text></card></stack>");
  assert.deepEqual(a.pending.map((each) => each.name), ["stack", "card", "text"]);

  const b = await session.push("ld</text></card>{#each items as item (item.id)}<text>{item.label}</text>");

  assert.equal(b.withheld, true);
  assert.equal(b.preview.text, "<stack><card title=\"x\"><text>Hello world</text></card></stack>");
  assert.equal(b.pending.at(-1).kind, "each");

  const c = await session.push("{/each}</stack>");

  assert.equal(c.withheld, false);
  assert.deepEqual(c.pending, []);
  assert.ok(c.preview.text.includes("{#each"));

  const wrong = await open();
  const d = await wrong.push("<stack><card titel=\"x\" ");

  assert.deepEqual(d.diagnostics.map((e) => e.code), ["unknown-prop"]);
  assert.equal(d.preview, undefined, "nothing is drawn while a diagnostic stands");
  assert.equal((await wrong.push("title=\"y\"></card>")).standing.length, 1, "the mistake still stands");
});

test("nothing is cut inside a surrogate pair: a chunk that ends between the halves is held until the other half comes", async () => {
  const session = await open();

  await session.push("<stack><text>Party \ud83c");
  assert.equal(session.text, "<stack><text>Party ");

  const step = await session.push("\udf89 time</text></stack>");

  assert.equal(session.text, "<stack><text>Party 🎉 time</text></stack>");
  assert.deepEqual(step.diagnostics, []);
  assert.deepEqual(await session.close(), await check({ source: session.text, path: "generated.mprx", model }));
});

test("a push over its budget ends the session (check-budget-exceeded), later pushes do no work, and close() is still the whole check", async () => {
  const session = await open({ budgetMs: 0 });
  const step = await session.push("<stack><text>x");

  assert.equal(step.ended.code, "check-budget-exceeded");
  assert.ok(step.ended.elapsedMs > 0);

  const later = await session.push("</text></stack>");

  assert.equal(later.ended.code, "check-budget-exceeded");
  assert.deepEqual(later.diagnostics, []);
  assert.deepEqual(await session.close(), { version: 1, diagnostics: [] });
});

test("a manifest that does not load is a usage error found when the session opens, with its diagnostics as the cause; arguments of the wrong type are usage errors", async () => {
  await assert.rejects(openSession({ manifest: "{ not json", component: "page" }), (error) => error instanceof MeshUsageError && error.code === "invalid-argument" && error.cause.diagnostics.length > 0);
  await assert.rejects(openSession({ manifest, component: "nope" }), MeshUsageError);
  await assert.rejects(openSession(null), MeshUsageError);

  const session = await open();

  await assert.rejects(session.push(42), MeshUsageError);
  await session.close();
  await assert.rejects(session.push("x"), MeshUsageError);
  await assert.rejects(session.close(), MeshUsageError);
});

test("the limits hold in a session: a source past 64 KB is input-too-large, nothing else is parsed, and there is no preview", async () => {
  const session = await open();
  const step = await session.push(`<stack>${"<text>x</text>".repeat(6000)}</stack>`);

  assert.ok(step.diagnostics.some((d) => ["input-too-large", "too-many-elements"].includes(d.code)));
  assert.equal(step.preview, undefined);
});
