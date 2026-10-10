// The documentation's own code samples run, with their imports pointed at the built packages. A sample may use
// names it doesn't define (`templates`, `manifest`, `draw`...): the test supplies them, and a sample that uses
// any other name fails with a ReferenceError, so a sample can't drift into inventing one.
//
// Not run here, and why: the browser sample in using-mesh-from-javascript.md ("In a browser") needs `location`
// and a served module; the guides' two full programs are run by guides.test.mjs.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { packageDir, root } from "./common.mjs";

const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
const modules = {
  "@valancex/mesh-compiler": await import(pathToFileURL(join(packageDir, "..", "mesh-compiler", "dist", "index.js")).href),
  "@valancex/mesh-runtime": await import(pathToFileURL(join(packageDir, "dist", "index.js")).href),
};

const jsBlocks = (path) => [...readFileSync(path, "utf8").matchAll(/```js\n([\s\S]*?)\n```/g)].map((match) => match[1]);

/** Runs a sample: its imports become the built packages; `scope` is what it may use besides them. */
async function run(sample, scope, tail = "") {
  const body = sample.replace(/^import\s+\{([^}]*)\}\s+from\s+"([^"]+)";?$/gm, (_, names, specifier) => {
    assert.ok(modules[specifier], `the sample imports only the two packages (${specifier})`);
    return `const {${names}} = __modules[${JSON.stringify(specifier)}];`;
  });
  const names = Object.keys(scope);
  return new AsyncFunction("__modules", ...names, `${body}\n${tail}`)(modules, ...names.map((name) => scope[name]));
}

const slice = (name) => readFileSync(join(root, "examples", "slice", name), "utf8");

async function sliceProgram() {
  const manifest = slice("components.json");
  const templates = [];
  for (const component of ["users", "user-card"]) {
    const result = await modules["@valancex/mesh-compiler"].compile({
      source: slice(`${component}.mprx`),
      path: `${component}.mprx`,
      model: { manifest, path: "components.json", component },
    });
    assert.ok(result.template, JSON.stringify(result.diagnostics));
    templates.push(JSON.stringify(result.template));
  }
  return { manifest, templates, snapshot: JSON.parse(slice("snapshots/first.json")) };
}

test("the runtime package's README sample renders, draws and dispatches", async () => {
  const [sample, ...others] = jsBlocks(join(packageDir, "README.md"));
  assert.equal(others.length, 0, "one sample; add its scope here when another is added");
  const { manifest, templates, snapshot } = await sliceProgram();
  const first = await modules["@valancex/mesh-runtime"].render({ program: { root: "users", templates }, model: manifest, snapshot });
  const handler = first.render.tree.root.children[0].children[0].events.click;
  const drawn = [];
  await run(sample, { templates, manifest, snapshot, draw: (tree) => drawn.push(tree), handler, payload: { x: 1, y: 2 } });
  assert.equal(drawn.length, 1, "draw was called with the tree");
  assert.equal(drawn[0].root.component, "page");
});

test("the compiler package's README sample checks a source against a manifest", async () => {
  const [sample, ...others] = jsBlocks(join(packageDir, "..", "mesh-compiler", "README.md"));
  assert.equal(others.length, 0, "one sample; add its scope here when another is added");
  const lines = [];
  await run(sample, { generated: slice("user-card.mprx"), manifestText: slice("components.json"), console: { log: (...args) => lines.push(args.join(" ")) } });
  assert.deepEqual(lines, [], "the slice's card checks clean, so the sample prints no diagnostic");
});

test("the JavaScript guide's samples check, and slice a diagnostic's span", async () => {
  const [checkSample, spanSample, repairSample, browserSample, ...others] = jsBlocks(join(root, "docs", "guides", "using-mesh-from-javascript.md"));
  assert.equal(others.length, 0, "four samples; add the new one's scope here");
  assert.ok(browserSample.includes("init("), "the last sample is the browser one, which needs a browser");
  const checked = await run(checkSample, {}, "return document;");
  assert.equal(checked.diagnostics[0].code, "mismatched-closing-tag");
  const source = "<page></pag>";
  const { check } = modules["@valancex/mesh-compiler"];
  const [diagnostic] = (await check({ source, path: "page.mprx" })).diagnostics;
  const text = await run(spanSample, { diagnostic, source }, "return source.slice(start.utf16, end.utf16);");
  assert.ok(text.length > 0 && source.includes(text), "the span is text of the source");
  assert.ok(repairSample.includes("check("), "the repair loop is run by the compiler's guide test");
});
