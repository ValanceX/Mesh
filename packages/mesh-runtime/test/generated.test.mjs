// Generated UI (Bet 13; G3, G11, G12): a preview is a fresh render of a checked template, and nothing unchecked is drawn. Streamed through the compiler's session byte by
// byte and rendered by the runtime, every preview must be a prefix of the final tree (a trailing text run may be shorter), an open {#each} must not appear before
// it closes, and once a diagnostic stands no later prefix has a preview. The programs and planted mistakes are examples/conformance/generated/.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { openSession } from "../../mesh-compiler/dist/index.js";
import { render } from "../dist/index.js";
import { root } from "./common.mjs";

const data = join(root, "examples", "conformance", "generated");
const manifest = readFileSync(join(data, "components.json"), "utf8");
const pages = Object.fromEntries(readdirSync(join(data, "pages")).sort().map((name) => [name.replace(/\.mprx$/, ""), readFileSync(join(data, "pages", name), "utf8")]));
const mistakes = JSON.parse(readFileSync(join(data, "mistakes.json"), "utf8"));
const items = [
  { id: 1, label: "one", done: true },
  { id: 2, label: "two", done: false },
  { id: 3, label: "three", done: false },
];
const snapshots = [
  { title: "Title", count: 0, items, showAll: true, mode: "compact" },
  { title: "Title", count: 3, items, showAll: false, mode: "roomy" },
  { title: "Title", count: 9, items: [], showAll: true, mode: "compact" },
];

const characters = (source) => [...source];

/** Whether `preview` is a prefix of `final`: the same nodes in the same order with the same props, and a trailing text run that may be shorter. Keys and handler ids differ by program. */
function prefixOf(preview, final, path = "root") {
  if (preview.type !== final.type) {
    return `${path}: ${preview.type} against ${final.type}`;
  }

  if (preview.type === "text") {
    return final.text.startsWith(preview.text) ? undefined : `${path}: the text ${JSON.stringify(preview.text)} is not a prefix of ${JSON.stringify(final.text)}`;
  }

  if (preview.component !== final.component) {
    return `${path}: <${preview.component}> against <${final.component}>`;
  }

  if (JSON.stringify(preview.props) !== JSON.stringify(final.props)) {
    return `${path}: the props of <${preview.component}> are ${JSON.stringify(preview.props)}, the final tree's ${JSON.stringify(final.props)}`;
  }

  const shown = preview.children ?? [];
  const all = final.children ?? [];

  if (shown.length > all.length) {
    return `${path}: <${preview.component}> has ${shown.length} children, the final tree's has ${all.length}`;
  }

  for (let i = 0; i < shown.length; i++) {
    // Only the last child may be unfinished; the others must be whole. A whole text child is equal, which the prefix test accepts.
    const problem = prefixOf(shown[i], all[i], `${path}/${i}`);

    if (problem !== undefined) {
      return problem;
    }

    if (i < shown.length - 1 && shown[i].type === "text" && shown[i].text !== all[i].text) {
      return `${path}/${i}: a text that is not the last child is cut`;
    }
  }

  return undefined;
}

const treeOf = async (template, snapshot) => {
  const result = await render({ program: { root: "page", templates: [JSON.stringify(template)] }, model: manifest, snapshot });

  assert.ok(result.render, `the template renders: ${JSON.stringify(result.diagnostics)}`);

  return result.render.tree.root;
};

test("every preview of every page, at every prefix, is a prefix of the final tree, in three states of the data", async () => {
  let previews = 0;

  for (const [name, source] of Object.entries(pages)) {
    const finalSession = await openSession({ manifest, component: "page" });
    const finalStep = await finalSession.push(source);

    assert.deepEqual(finalStep.standing, [], name);
    assert.ok(finalStep.preview, `${name}: the whole page has a preview`);

    const finals = [];

    for (const snapshot of snapshots) {
      finals.push(await treeOf(finalStep.preview.template, snapshot));
    }

    let text = "";
    const session = await openSession({ manifest, component: "page" });

    for (const character of characters(source)) {
      text += character;

      const step = await session.push(character);

      assert.deepEqual(step.standing, [], `${name}: ${JSON.stringify(text)}`);

      if (step.preview === undefined) {
        continue;
      }

      previews += 1;

      for (const [index, snapshot] of snapshots.entries()) {
        const problem = prefixOf(await treeOf(step.preview.template, snapshot), finals[index], "root");

        assert.equal(problem, undefined, `${name}, state ${index}, after ${JSON.stringify(text)}: ${problem}`);
      }

      // An {#each} that is open is not shown: the preview never holds a row of it.
      if (step.pending.some((each) => each.kind === "each")) {
        assert.ok(!step.preview.text.includes("{#each"), `${name}: an open {#each} is in the preview`);
      }
    }
  }

  assert.ok(previews > 800, `${previews} previews were rendered`);
});

test("a preview is a different program from the next one: no node keeps its key (the reason every step is a fresh render, G11)", async () => {
  const session = await openSession({ manifest, component: "page" });
  const a = await session.push("<stack><text>one</text>");
  const b = await session.push("<text>two</text></stack>");
  const first = await treeOf(a.preview.template, snapshots[0]);
  const second = await treeOf(b.preview.template, snapshots[0]);

  assert.notEqual(first.key, second.key);
  assert.notEqual(first.children[0].key, second.children[0].key);
});

test("once a diagnostic stands no later prefix has a preview, and nothing wrong is ever in one: the planted mistakes", async () => {
  for (const { name, code, source } of mistakes) {
    const session = await openSession({ manifest, component: "page" });
    let standing = false;
    let text = "";

    for (const character of characters(source)) {
      text += character;

      const step = await session.push(character);

      if (step.standing.some((d) => d.severity === "error")) {
        standing = true;
      }

      if (standing) {
        assert.equal(step.preview, undefined, `${name}: a preview after ${JSON.stringify(text)} with ${code} standing`);
      }
    }

    assert.ok(standing, `${name}: ${code} was found`);
  }
});
