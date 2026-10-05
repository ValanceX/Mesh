// The module runs in a browser, through the explicit initialization path
// and through the automatic one (no `init`: the packaged module next to
// the runtime's code), and answers what it answers in Node. Chromium is
// Playwright's, or `MESH_CHROMIUM` if set.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cp, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
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

// `moved` hides the packaged module from where the runtime looks for it
// (`/dist/mesh-runtime.wasm`) and offers it at `/elsewhere/mesh-runtime.wasm`.
function serve({ page = PAGE, moved = false } = {}) {
  const requested = [];
  const server = createServer(async (request, response) => {
    try {
      const path = new URL(request.url, "http://localhost").pathname;
      requested.push(path);
      if (path === "/") {
        response.writeHead(200, { "content-type": TYPES[".html"] }).end(page);
        return;
      }
      if (moved && path === "/dist/mesh-runtime.wasm") {
        response.writeHead(404).end();
        return;
      }
      const file = normalize(join(packageDir, moved && path === "/elsewhere/mesh-runtime.wasm" ? "/dist/mesh-runtime.wasm" : path));
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
  server.requested = requested;
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

// A page with no `init`: the runtime finds its own module.
const PAGE_AUTOMATIC = `<!doctype html><meta charset="utf-8"><script type="module">
  import { render, dispatch } from "/dist/index.js";
  window.inPage = async ({ program, model, snapshot }) => {
    const made = await render({ program, model, snapshot });
    const find = (node) =>
      node.events.tap ?? node.children.filter((c) => c.type === "node").map(find).find(Boolean);
    const tap = find(made.render.tree.root);
    const { intent } = await dispatch(made.render, tap);
    return { tree: made.render.tree, intent, tap };
  };
  window.ready = true;
</script>`;

// A page whose packaged module is elsewhere: `init` is given that URL, or is not called.
const PAGE_MOVED = `<!doctype html><meta charset="utf-8"><script type="module">
  import { init, render } from "/dist/index.js";
  window.explicit = async ({ program, model, snapshot }) => {
    await init(new URL("/elsewhere/mesh-runtime.wasm", location.href));
    return (await render({ program, model, snapshot })).render.tree;
  };
  window.automatic = async ({ program, model, snapshot }) => {
    try {
      await render({ program, model, snapshot });
      return { rendered: true };
    } catch (error) {
      return { message: String(error.message), cause: String(error.cause?.message ?? error.cause ?? "") };
    }
  };
  window.ready = true;
</script>`;

async function inChromium(server, run) {
  const browser = await chromium.launch(
    process.env.MESH_CHROMIUM ? { executablePath: process.env.MESH_CHROMIUM } : {},
  );
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => window.ready === true);
    await run(page);
    return errors;
  } finally {
    await browser.close();
    server.close();
  }
}

test("a browser without init() loads the packaged module next to the runtime, and answers as with init()", { timeout: 120_000 }, async () => {
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const made = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const server = await serve({ page: PAGE_AUTOMATIC });
  const errors = await inChromium(server, async (page) => {
    const result = await page.evaluate((input) => window.inPage(input), { program, model: MODEL, snapshot: SNAPSHOT });
    assert.deepEqual(result.tree, made.render.tree);
    assert.deepEqual(result.intent, (await dispatch(made.render, result.tap)).intent);
  });
  assert.deepEqual(errors, []);
  assert.ok(server.requested.includes("/dist/mesh-runtime.wasm"), "the runtime asked for the module beside its code");
});

test("an explicit init(url) is authoritative: that URL is used, and the runtime never looks beside its code", { timeout: 120_000 }, async () => {
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const made = await render({ program, model: MODEL, snapshot: SNAPSHOT });
  const server = await serve({ page: PAGE_MOVED, moved: true });
  const errors = await inChromium(server, async (page) => {
    const tree = await page.evaluate((input) => window.explicit(input), { program, model: MODEL, snapshot: SNAPSHOT });
    assert.deepEqual(tree, made.render.tree);
  });
  assert.deepEqual(errors, []);
  assert.ok(server.requested.includes("/elsewhere/mesh-runtime.wasm"));
  assert.ok(!server.requested.includes("/dist/mesh-runtime.wasm"), "no automatic lookup when init() was called");
});

test("a browser that cannot load the packaged module without init() gets an actionable error that keeps the original failure", { timeout: 120_000 }, async () => {
  const program = { root: "view", templates: await programTemplates(join(programsDir, "cards")) };
  const server = await serve({ page: PAGE_MOVED, moved: true });
  await inChromium(server, async (page) => {
    const result = await page.evaluate((input) => window.automatic(input), { program, model: MODEL, snapshot: SNAPSHOT });
    assert.equal(result.rendered, undefined);
    assert.match(result.message, /could not load its packaged WebAssembly module automatically/);
    assert.match(result.message, /404/, "the original failure is in the message");
    assert.match(result.cause, /404/, "and is the error's cause");
    assert.match(result.message, /init\(\)/, "names the explicit way");
    assert.match(result.message, /Vite 5-7 development server, it may have moved/, "names the Vite 5-7 dev case, as a possibility");
    assert.match(result.message, /optimizeDeps\.exclude/);
    assert.doesNotMatch(result.message, /call init\(\) with the URL of mesh-runtime\.wasm before the first render/, "the old refusal is gone");
  });
});

test("Node needs no init(): the packaged module loads, and a missing module fails with the file error, not the browser message", { timeout: 60_000 }, async () => {
  const script = `import { render } from ${JSON.stringify(new URL("../dist/index.js", import.meta.url).href)};
    const r = await render({ program: { root: "view", templates: JSON.parse(process.argv[1]) }, model: process.argv[2], snapshot: JSON.parse(process.argv[3]) });
    process.stdout.write(r.diagnostics ? "DIAGNOSTICS" : "RENDERED");`;
  const templates = await programTemplates(join(programsDir, "cards"));
  const run = (file) => execFileSync(process.execPath, ["--input-type=module", "-e", script.replace(new URL("../dist/index.js", import.meta.url).href, file), JSON.stringify(templates), MODEL, JSON.stringify(SNAPSHOT)], { encoding: "utf8" });

  assert.equal(run(new URL("../dist/index.js", import.meta.url).href), "RENDERED");

  const copy = await mkdtemp(join(tmpdir(), "mesh-runtime-"));
  try {
    await cp(join(packageDir, "dist"), copy, { recursive: true });
    await rm(join(copy, "mesh-runtime.wasm"));
    assert.throws(() => run(new URL(`file://${copy}/index.js`).href), (error) => /ENOENT/.test(String(error.stderr)) && !/automatically/.test(String(error.stderr)));
  } finally {
    await rm(copy, { recursive: true, force: true });
  }
});
