// `source.slice(start.utf16, end.utf16)` is the text a span covers
// (outline v0.4 D5): for every corpus run, and for sources with
// multi-byte UTF-8, astral characters, CRLF and a leading BOM.
import assert from "node:assert/strict";
import { test } from "node:test";
import { check } from "../dist/index.js";
import { corpus, input, read } from "./common.mjs";

function spans(document) {
  return document.diagnostics.flatMap((diagnostic) => [
    diagnostic.span,
    ...diagnostic.suggestions.map((suggestion) => suggestion.span),
  ]);
}

async function assertSlices(value) {
  const document = await check(value);
  // The span's text is in the manifest for a manifest-* code.
  for (const diagnostic of document.diagnostics) {
    const text = diagnostic.path === value.path ? value.source : value.model.manifest;
    const bytes = Buffer.from(text, "utf8");
    for (const span of spans({ diagnostics: [diagnostic] })) {
      assert.equal(
        text.slice(span.start.utf16, span.end.utf16),
        bytes.subarray(span.start.byte, span.end.byte).toString("utf8"),
        `${value.path}: ${JSON.stringify(span)}`,
      );
    }
  }
  return document;
}

test("every corpus span slices the same text in UTF-16 as in bytes", async () => {
  for (const run of corpus()) {
    await assertSlices(input(run));
  }
});

test("multi-byte, astral, CRLF and BOM sources slice exactly", async () => {
  const manifest = read("fixtures/check/components.json");
  const emoji = read("fixtures/check/fail/unknown-reference-after-emoji.mprx");
  const variants = [
    emoji,
    `﻿${emoji}`,
    emoji.replaceAll("\n", "\r\n"),
    `﻿${emoji.replaceAll("\n", "\r\n")}`,
    "﻿<a>\r\n  <b title=\"日本\">😀 {x}</c>\r\n</a>",
    "<p>Größe 😀</q>",
  ];
  let checked = 0;
  for (const source of variants) {
    const document = await assertSlices({
      source,
      path: "f.mprx",
      model: { manifest, path: "components.json", component: "template" },
    });
    checked += spans(document).length;
  }
  assert.ok(checked >= variants.length, `only ${checked} spans`);
});
