// Every kind of JavaScript value reaches the runtime, which judges it
// (spec §9.8.6, the Definition of Done's "Inputs"): the package encodes
// structurally and decides nothing, so each gets the runtime's
// diagnostic, at its path.
import assert from "node:assert/strict";
import { test } from "node:test";
import { render } from "../dist/index.js";
import { MODEL, SNAPSHOT, template } from "./common.mjs";

const program = { root: "view", templates: [await template("view", "<text>{name}</text>")] };

/** The diagnostics for SNAPSHOT with `changes` applied, as [code, path] pairs. */
async function problems(changes) {
  const result = await render({ program, model: MODEL, snapshot: { ...SNAPSHOT, ...changes } });
  return result.diagnostics
    ? result.diagnostics.diagnostics.map((d) => [d.code, d.location.path])
    : [];
}

class Point {
  constructor() {
    this.x = 1;
  }
}

test("each kind of value gets the runtime's diagnostic, at its path", async () => {
  const cycle = { name: "Ada" };
  cycle.self = cycle;
  const cases = [
    [{ numbers: [1, undefined] }, [["runtime-absent-element", ["numbers", 1]]]],
    // eslint-disable-next-line no-sparse-arrays
    [{ numbers: [1, , 2] }, [["runtime-absent-element", ["numbers", 1]]]],
    [{ count: NaN }, [["runtime-non-finite-input", ["count"]]]],
    [{ count: -Infinity }, [["runtime-non-finite-input", ["count"]]]],
    [{ count: 10n }, [["runtime-unsupported-value", ["count"]]]],
    [{ anything: () => 1 }, [["runtime-unsupported-value", ["anything"]]]],
    [{ anything: Symbol("s") }, [["runtime-unsupported-value", ["anything"]]]],
    [{ anything: new Map() }, [["runtime-unsupported-value", ["anything"]]]],
    [{ anything: new Date(0) }, [["runtime-unsupported-value", ["anything"]]]],
    [{ anything: new Point() }, [["runtime-unsupported-value", ["anything"]]]],
    [{ anything: cycle }, [["runtime-unsupported-value", ["anything", "self"]]]],
    [{ anything: { deep: [cycle] } }, [["runtime-unsupported-value", ["anything", "deep", 0, "self"]]]],
    [{ name: "a\ud800" }, [["runtime-unpaired-surrogate", ["name"]]]],
    [{ user: { name: "Ada", active: true, extra: 1 } }, [["runtime-unknown-field", ["user", "extra"]]]],
    [{ count: "3" }, [["runtime-value-mismatch", ["count"]]]],
    [{ nothing: undefined }, [["runtime-missing-value", ["nothing"]]]],
  ];
  for (const [changes, expected] of cases) {
    assert.deepEqual(await problems(changes), expected, JSON.stringify(Object.keys(changes)));
  }
});

test("absence, -0, astral characters and null-prototype objects cross as they are", async () => {
  assert.deepEqual(
    await problems({
      maybeName: undefined,
      count: -0,
      name: "😀 𝄞",
      user: Object.assign(Object.create(null), { name: "Ada", active: true, avatar: undefined }),
      anything: { a: [null, true, "", 1e-7] },
      extra: new Map(), // not in the root's scope: ignored entirely
    }),
    [],
  );
});

test("a repeated value that isn't a cycle is encoded twice, and is fine", async () => {
  const shared = { name: "Ada", active: true };
  assert.deepEqual(await problems({ user: shared, users: [shared, shared] }), []);
});

test("a message names the kind of an unsupported value", async () => {
  const result = await render({ program, model: MODEL, snapshot: { ...SNAPSHOT, anything: new Map() } });
  assert.match(result.diagnostics.diagnostics[0].message, /`Map` object/);
});
