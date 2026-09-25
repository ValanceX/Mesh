// The public API's contract (outline v0.4 D4).
import assert from "node:assert/strict";
import { test } from "node:test";
import { check, version } from "../dist/index.js";
import { read } from "./common.mjs";

test("checks a clean file", async () => {
  const document = await check({ source: read("user-card.mprx"), path: "user-card.mprx" });
  assert.deepEqual(document, { version: 1, diagnostics: [] });
});

test("checks against a model", async () => {
  const document = await check({
    source: read("users-page.mprx"),
    path: "users-page.mprx",
    model: { manifest: read("components.json"), path: "components.json", component: "users-page" },
  });
  assert.deepEqual(document.diagnostics, []);
});

test("reports a broken manifest on the manifest", async () => {
  const document = await check({
    source: "<a",
    path: "a.mprx",
    model: { manifest: "{", path: "broken.json", component: "a" },
  });
  assert.ok(document.diagnostics.length > 0);
  for (const diagnostic of document.diagnostics) {
    assert.equal(diagnostic.path, "broken.json");
    assert.match(diagnostic.code, /^manifest-/);
  }
});

test("a problem with the input is a diagnostic, not an exception", async () => {
  const document = await check({ source: "<a></b>", path: "a.mprx" });
  assert.equal(document.diagnostics[0].code, "mismatched-closing-tag");
});

test("a model needs a component, and every input is a string", async () => {
  await assert.rejects(
    check({ source: "<a />", path: "a.mprx", model: { manifest: "{}", path: "m.json" } }),
    TypeError,
  );
  await assert.rejects(check({ source: 1, path: "a.mprx" }), TypeError);
  await assert.rejects(check(null), TypeError);
});

test("paths are never read", async () => {
  const path = "/no/such/directory/page.mprx";
  const document = await check({ source: "<a></b>", path });
  assert.equal(document.diagnostics[0].path, path);
});

test("the version is the package's", async () => {
  const pkg = JSON.parse(read("../packages/mesh-compiler/package.json"));
  assert.equal(version, pkg.version);
});
