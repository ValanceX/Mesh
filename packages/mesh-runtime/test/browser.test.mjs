// The module runs in a browser, through the explicit initialization path,
// and answers what it answers in Node. Chromium is Playwright's, or
// `MESH_CHROMIUM` if set.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { test } from "node:test";
import { chromium } from "playwright";
import { dispatch, render } from "../dist/index.js";
import { MODEL, packageDir, programsDir, programTemplates, SNAPSHOT } from "./common.mjs";

const TYPES = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };

const PAGE = `<!doctype html><meta charset="utf-8"><script type="module">
  import { init, render, dispatch } from "/dist/index.js";
  window.inPage = async ({ program, model, snapshot }) => {
    await init(new URL("/dist/mesh-runtime.wasm", location.href));
    const made = await render({ program, model, snapshot });
    const refused = await render({ program, model, snapshot: { ...snapshot, anything: new Map() } });
    const find = (node) =>
      node.events.tap ?? node.children.filter((c) => c.type === "node").map(find).find(Boolean);
    const tap = find(made.render.tree.root);
    const { intent } = await dispatch(made.render, tap);
    return { tree: made.render.tree, refused: refused.diagnostics, intent, tap };
  };
  window.ready = true;
</script>`;

function serve() {
  const server = createServer(async (request, response) => {
    try {
      const path = new URL(request.url, "http://localhost").pathname;
      if (path === "/") {
        response.writeHead(200, { "content-type": TYPES[".html"] }).end(PAGE);
        return;
      }
      const file = normalize(join(packageDir, path));
      if (!file.startsWith(join(packageDir, "dist"))) {
        response.writeHead(404).end();
        return;
      }
      const body = await readFile(file);
      response.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

test("renders and dispatches in headless Chromium, as in Node", { timeout: 120_000 }, async () => {
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const made = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const refused = await render({ program, model: MODEL, snapshot: { ...SNAPSHOT, anything: new Map() } });
  const server = await serve();
  let browser;
  try {
    browser = await chromium.launch(
      process.env.MESH_CHROMIUM ? { executablePath: process.env.MESH_CHROMIUM } : {},
    );
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => window.ready === true);
    const result = await page.evaluate((input) => window.inPage(input), { program, model: MODEL, snapshot: SNAPSHOT });
    assert.deepEqual(result.tree, made.render.tree);
    assert.deepEqual(result.refused, refused.diagnostics);
    assert.deepEqual(result.intent, (await dispatch(made.render, result.tap)).intent);
    assert.deepEqual(errors, []);
  } finally {
    await browser?.close();
    server.close();
  }
});
