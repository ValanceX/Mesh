// The module runs in a browser, through the explicit initialization path
// (outline v0.4 D4), and answers what `mesh check` answers. Chromium is
// Playwright's, or `MESH_CHROMIUM` if set.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { test } from "node:test";
import { chromium } from "playwright";
import { cli, packageDir, read } from "./common.mjs";

const TYPES = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };

const PAGE = `<!doctype html><meta charset="utf-8"><script type="module">
  import { init, check } from "/dist/index.js";
  window.checkInPage = async (input) => {
    await init(new URL("/dist/mesh.wasm", location.href));
    return check(input);
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

test("checks in headless Chromium, as mesh check does", { timeout: 120_000 }, async () => {
  const server = await serve();
  const browser = await chromium.launch(
    process.env.MESH_CHROMIUM ? { executablePath: process.env.MESH_CHROMIUM } : {},
  );
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => window.ready === true);
    const file = "fixtures/fail/mismatched-closing-tag.mprx";
    const document = await page.evaluate((input) => window.checkInPage(input), { source: read(file), path: file });
    assert.deepEqual(document, cli(file));
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
