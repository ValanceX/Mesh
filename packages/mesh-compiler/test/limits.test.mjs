// The input limits (decision G5) hold in WebAssembly, and what is within them
// is checked in time that grows with its size, not with its square: a source
// over 64 KB, more than 5,000 elements, or more than 64 attributes on an
// element is one diagnostic at the first place past it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { check } from "../dist/index.js";

const MANIFEST = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: { props: {}, events: {}, commands: {}, scope: {} },
    t: { props: {}, events: {}, commands: {}, scope: {} },
  },
});
const model = { manifest: MANIFEST, path: "m.json", component: "page" };
const codes = async (source) =>
  (await check({ source, path: "g.mprx", model })).diagnostics.map((d) => d.code);

test("a source past 64 KB is input-too-large, at the first byte past it", async () => {
  const at = `<page>${"x".repeat(65_536 - 13)}</page>`;
  assert.equal(at.length, 65_536);
  assert.deepEqual(await codes(at), []);
  const past = `<page>${"x".repeat(65_536 - 12)}</page>`;
  const document = await check({ source: past, path: "g.mprx", model });
  assert.deepEqual(document.diagnostics.map((d) => d.code), ["input-too-large"]);
  assert.equal(document.diagnostics[0].span.start.utf16, 65_536);
});

test("5,000 elements check and 5,001 are too-many-elements", async () => {
  const siblings = (count) => `<page>${"<t/>".repeat(count - 1)}</page>`;
  assert.deepEqual(await codes(siblings(5_000)), []);
  assert.deepEqual(await codes(siblings(5_001)), ["too-many-elements"]);
});

test("64 attributes check and 65 are too-many-attributes", async () => {
  const attributes = (count) =>
    `<t ${Array.from({ length: count }, (_, i) => `p${i}="x"`).join(" ")} />`;
  assert.ok(!(await codes(attributes(64))).includes("too-many-attributes"));
  assert.deepEqual(await codes(attributes(65)), ["too-many-attributes"]);
});

// A ratio, not a time, as wall-clock numbers vary between runs and machines: the
// least of three runs of four times the siblings takes less than ten times as
// long (linear is four, quadratic sixteen). Before the source map was fixed,
// 4,000 siblings took about 25 times as long as 1,000.
test("siblings are checked in linear time", async () => {
  const least = async (count) => {
    const source = `<page>${"<t>é</t>".repeat(count)}</page>`;
    let best = Infinity;
    for (let run = 0; run < 3; run++) {
      const started = performance.now();
      await check({ source, path: "g.mprx", model });
      best = Math.min(best, performance.now() - started);
    }
    return best;
  };
  await least(250);
  const small = await least(1_000);
  const large = await least(4_000);
  assert.ok(large < 10 * small, `1,000 siblings ${small.toFixed(0)} ms, 4,000 ${large.toFixed(0)} ms`);
});
