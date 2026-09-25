// The differential number-to-text test against V8 (Pass 3 Decision 13a;
// outline D8). Compatibility and regression evidence, NOT the normative
// proof: the normative table (`docs/tables/number-to-text.tsv`) is that,
// and V8 is never the authority for MESH's output. ECMA-262 leaves the
// last digit open, so a conforming engine may legitimately differ.
//
// For each value, MESH's text from the WebAssembly runtime and V8's
// `String(x)` are compared as strings. Each difference is triaged by
// exact arithmetic on BigInts (test code only, and not normative):
//
// - `mesh-bug`: MESH's text doesn't round-trip; or is longer than a
//   round-tripping V8 text; or, at equal length, V8's is strictly
//   nearer, or equally near with an even last digit where MESH's is odd.
//   This fails.
// - `v8-differs`: MESH's text satisfies §9.7 and V8's doesn't (longer,
//   farther, or the odd one of a tie). This fails unless the value is
//   listed in `number-text-differences.json`, reviewed like code.
// - `unclassified`: anything else. This fails, with both texts and the
//   classifier's measurements.
//
// `MESH_NUMBER_VALUES` (default 1,000,000) and `MESH_NUMBER_SEED`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { Module, numbersRequest } from "./common/module.mjs";
import { here, prng, testModule } from "./common.mjs";

const module = new Module(testModule);

// --- bits ---------------------------------------------------------------

const scratch = new DataView(new ArrayBuffer(8));

function fromBits(bits) {
  scratch.setBigUint64(0, bits);
  return scratch.getFloat64(0);
}

function bitsOf(x) {
  scratch.setFloat64(0, x);
  return scratch.getBigUint64(0);
}

const hex = (x) => bitsOf(x).toString(16).padStart(16, "0");

/** `x` moved by `ulps` units in the last place, toward +∞ for positive ulps (for x ≥ 0). */
function ulpStep(x, ulps) {
  const moved = bitsOf(x) + BigInt(ulps);
  if (moved < 0n || moved >= 0x7ff0000000000000n) return x;
  return fromBits(moved);
}

// --- exact triage ------------------------------------------------------

/** `x`, finite and nonzero, as an exact m·2^e with m a BigInt. */
function exact(x) {
  const bits = bitsOf(Math.abs(x));
  const exponent = Number(bits >> 52n);
  const fraction = bits & 0xfffffffffffffn;
  return exponent === 0
    ? { m: fraction, e: -1074 }
    : { m: fraction | 0x10000000000000n, e: exponent - 1075 };
}

/** A decimal text, as its digits (no leading or trailing zeros) and exponent: d·10^k. */
function decimal(text) {
  const match = /^-?(\d+)(?:\.(\d+))?(?:e([+-]\d+))?$/.exec(text);
  if (!match) return undefined;
  const [, whole, fraction = "", exp = "0"] = match;
  let digits = (whole + fraction).replace(/^0+/, "");
  let k = Number(exp) - fraction.length;
  if (digits === "") return { digits: "0", k: 0 };
  while (digits.endsWith("0")) {
    digits = digits.slice(0, -1);
    k += 1;
  }
  return { digits, k };
}

/** |d·10^k − m·2^e|, scaled by a positive factor common to every text for one value. */
function distance(x, { digits, k }) {
  const { m, e } = exact(x);
  // Scale both sides by 2^1074 · 10^400, enough to make every term an integer.
  const d = BigInt(digits) * 10n ** BigInt(k + 400) * 2n ** 1074n;
  const v = m * 2n ** BigInt(e + 1074) * 10n ** 400n;
  return d > v ? d - v : v - d;
}

/** The classifier's measurements of one text for `x`. */
export function measure(x, text) {
  const parsed = decimal(text);
  return {
    text,
    roundTrips: Number(text) === x || (Object.is(Number(text), -0) && x === 0),
    length: parsed ? parsed.digits.length : Infinity,
    distance: parsed ? distance(x, parsed) : undefined,
    even: parsed ? Number(parsed.digits.at(-1)) % 2 === 0 : false,
  };
}

/** `mesh-bug`, `v8-differs` or `unclassified`, for texts that differ. */
export function classify(x, meshText, v8Text) {
  const mesh = measure(x, meshText);
  const v8 = measure(x, v8Text);
  if (!mesh.roundTrips) return { kind: "mesh-bug", mesh, v8 };
  if (v8.roundTrips && v8.length < mesh.length) return { kind: "mesh-bug", mesh, v8 };
  if (v8.roundTrips && v8.length === mesh.length) {
    if (v8.distance < mesh.distance) return { kind: "mesh-bug", mesh, v8 };
    if (v8.distance === mesh.distance && v8.even && !mesh.even && v8.distance !== 0n) {
      return { kind: "mesh-bug", mesh, v8 };
    }
  }
  const v8Worse =
    !v8.roundTrips ||
    v8.length > mesh.length ||
    (v8.length === mesh.length && v8.distance > mesh.distance) ||
    (v8.length === mesh.length && v8.distance === mesh.distance && mesh.even && !v8.even);
  if (v8Worse) return { kind: "v8-differs", mesh, v8 };
  return { kind: "unclassified", mesh, v8 };
}

// --- values -------------------------------------------------------------

const STRATA = [
  ["uniform random bit patterns", 300_000],
  ["integers", 150_000],
  ["short decimals and their neighbours", 150_000],
  ["15-17 significant digits", 100_000],
  ["powers of two ±0-3 ulps", 100_000],
  ["powers of ten ±0-3 ulps", 100_000],
  ["subnormals", 100_000],
];

function generate(total, seed) {
  const random = prng(seed);
  const u32 = () => BigInt(Math.floor(random() * 2 ** 32));
  const bits64 = () => (u32() << 32n) | u32();
  const int = (n) => Math.floor(random() * n);
  const digitsOf = (count) => {
    let text = String(1 + int(9));
    while (text.length < count) text += String(int(10));
    return text;
  };
  const scale = total / 1_000_000;
  const values = [];
  const counts = {};
  for (const [name, count] of STRATA) {
    const n = Math.max(1, Math.round(count * scale));
    counts[name] = n;
    for (let index = 0; index < n; index++) {
      let x;
      switch (name) {
        case "uniform random bit patterns":
          do x = fromBits(bits64()); while (!Number.isFinite(x));
          break;
        case "integers":
          x = random() < 0.5
            ? Number(((u32() << 21n) | (u32() >> 11n)) % 2n ** 53n) + 1
            : Math.floor(10 ** (random() * 22));
          break;
        case "short decimals and their neighbours": {
          const base = Number(`${digitsOf(1 + int(17))}e${int(61) - 30}`);
          x = [base, ulpStep(base, 1), ulpStep(base, -1)][index % 3];
          break;
        }
        case "15-17 significant digits":
          x = Number(`${digitsOf(15 + int(3))}e${int(600) - 300}`);
          break;
        case "powers of two ±0-3 ulps":
          x = ulpStep(2 ** (int(2098) - 1074), int(7) - 3);
          break;
        case "powers of ten ±0-3 ulps":
          x = ulpStep(Number(`1e${int(632) - 323}`), int(7) - 3);
          break;
        default: {
          const pick = int(10);
          x = pick === 0 ? 5e-324 : pick === 1 ? fromBits(0x000fffffffffffffn) : fromBits(bits64() & 0x000fffffffffffffn);
        }
      }
      if (!Number.isFinite(x) || x === 0) x = 5e-324;
      values.push(random() < 0.5 ? -x : x);
    }
  }
  return { values, counts };
}

/** MESH's texts for `values`, from the WebAssembly runtime, in batches. */
function meshTexts(values) {
  const texts = [];
  for (let start = 0; start < values.length; start += 100_000) {
    const { status, result } = module.request(numbersRequest(values.slice(start, start + 100_000)));
    assert.equal(status, 0);
    texts.push(...result.split("\n").slice(0, -1));
  }
  return texts;
}

// --- the tests ----------------------------------------------------------

test("the classifier classifies known cases correctly, in each direction", () => {
  const x = 0.1;
  // MESH's text made one digit longer (and farther): a MESH bug.
  assert.equal(classify(x, "0.10000000000000001", "0.1").kind, "mesh-bug");
  // MESH's text that doesn't round-trip: a MESH bug.
  assert.equal(classify(x, "0.2", "0.1").kind, "mesh-bug");
  // V8's replaced by a farther candidate of the same length: V8 differs.
  const y = 5e-324; // 4.94…e-324: "5e-324" and "4e-324" both round-trip
  assert.equal(classify(y, "5e-324", "4e-324").kind, "v8-differs");
  // V8's longer: V8 differs.
  assert.equal(classify(x, "0.1", "0.10000000000000001").kind, "v8-differs");
  // The same digits laid out differently: unclassified.
  assert.equal(classify(1e21, "1e+21", "1000000000000000000000").kind, "unclassified");
});

test("MESH's text agrees with V8's, or every difference is known", { timeout: 30 * 60 * 1000 }, () => {
  const total = Number(process.env.MESH_NUMBER_VALUES ?? 1_000_000);
  const seed = Number(process.env.MESH_NUMBER_SEED ?? 0x6e756d73);
  const known = new Map(
    JSON.parse(readFileSync(join(here, "number-text-differences.json"), "utf8")).map((entry) => [entry.bits, entry]),
  );
  const { values, counts } = generate(total, seed);
  const texts = meshTexts(values);
  assert.equal(texts.length, values.length);
  const failures = [];
  const found = { "mesh-bug": 0, "v8-differs": 0, unclassified: 0, known: 0 };
  for (let index = 0; index < values.length; index++) {
    const x = values[index];
    const v8 = String(x);
    if (texts[index] === v8) continue;
    const result = classify(x, texts[index], v8);
    const listed = known.get(hex(x));
    if (result.kind === "v8-differs" && listed && listed.mesh === texts[index] && listed.v8 === v8) {
      found.known += 1;
      continue;
    }
    found[result.kind] += 1;
    if (failures.length < 20) {
      const { mesh, v8: theirs } = result;
      const show = (m) => `${m.text} (round-trips ${m.roundTrips}, ${m.length} digits, distance ${m.distance})`;
      failures.push(`${hex(x)} ${result.kind}: MESH ${show(mesh)}; V8 ${show(theirs)}`);
    }
  }
  console.log(
    `number-text-v8: ${values.length} values of seed ${seed}, Node ${process.version}, V8 ${process.versions.v8}; ` +
      `strata ${JSON.stringify(counts)}; differences ${JSON.stringify(found)}`,
  );
  assert.deepEqual(failures, [], "every difference is classified and known");
});
