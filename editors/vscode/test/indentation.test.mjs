// The indentation rules in language-configuration.json, run on the lines an author types: what VS Code does on Enter and when a closing line is typed. They are line patterns: they cannot know a
// scope the way mesh-lsp's formatter does (which then corrects the line, as `editor.formatOnType` is on for MPRX); they are what applies at once, and what applies without the server.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const config = JSON.parse(readFileSync(fileURLToPath(new URL("../language-configuration.json", import.meta.url)), "utf8"));
const increase = new RegExp(config.indentationRules.increaseIndentPattern);
const decrease = new RegExp(config.indentationRules.decreaseIndentPattern);

test("a line that opens a scope indents the next one", () => {
  for (const line of ["<page>", '  <section class="a">', "  <button", "  >", "{#if ready}", "  {#each xs as x (x.id)}", "{:else}", "{:else if a && b}", "<a href={x > y}>"]) {
    assert.ok(increase.test(line), line);
  }
});

test("a line that opens and closes, or only fills a scope, does not", () => {
  for (const line of ["<a />", "<a href=\"x\" />", "<b>x</b>", "<p>text {name}</p>", "{count}", "text only", "  class=\"theme\"", "{/if}", "</page>", "/>"]) {
    assert.ok(!increase.test(line), line);
  }
});

test("a line that closes a scope is outdented", () => {
  for (const line of ["  </section>", "</page>", "  {/if}", "{:else}", "  {:else if a}", "  >", "  />", "/>"]) {
    assert.ok(decrease.test(line), line);
  }
});

test("a line that does not close is not outdented", () => {
  for (const line of ["<a>", "{#if a}", "{count}", "text", "  class=\"x\""]) {
    assert.ok(!decrease.test(line), line);
  }
});

test("Enter between a tag and its closer, or a block opener and its closer, indents the new line and outdents the closer", () => {
  const [tag, block] = config.onEnterRules;

  assert.ok(new RegExp(tag.beforeText).test("  <section>") && new RegExp(tag.afterText).test("</section>"));
  assert.ok(!new RegExp(tag.beforeText).test("  <a />"));
  assert.ok(new RegExp(block.beforeText).test("{#if a}") && new RegExp(block.afterText).test("{/if}"));
  assert.equal(tag.action.indent, "indentOutdent");
  assert.equal(block.action.indent, "indentOutdent");
});

test("the extension turns on what the rules need", () => {
  const defaults = JSON.parse(readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8")).contributes.configurationDefaults["[mprx]"];

  assert.equal(defaults["editor.autoIndent"], "full");
  assert.equal(defaults["editor.formatOnType"], true);
});
