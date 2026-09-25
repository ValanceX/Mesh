// The test host and the reference renderer, in Node (D10): every program
// under `crates/mesh-runtime/tests/programs/` renders, prints, compares
// and dispatches exactly as the committed files, which the native test
// host produced, say.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { Host } from "./common/host.mjs";
import { compare, print } from "./common/renderer.mjs";
import { MODEL, programDirs, programTemplates } from "./common.mjs";

test("every program renders and dispatches as committed", async () => {
  const dirs = programDirs();
  assert.ok(dirs.length >= 2);
  for (const dir of dirs) {
    const host = new Host({ model: MODEL, root: "view", templates: await programTemplates(dir) });
    const snapshots = readdirSync(join(dir, "snapshots")).filter((f) => f.endsWith(".json")).sort();
    for (const [index, file] of snapshots.entries()) {
      const path = join(dir, "snapshots", file);
      const made = await host.render(JSON.parse(readFileSync(path, "utf8")));
      assert.equal(print(made.tree), readFileSync(path.replace(/\.json$/, ".html"), "utf8"), path);
      if (index > 0) {
        assert.equal(
          compare(host.renders[index - 1].tree, made.tree),
          readFileSync(path.replace(/\.json$/, ".changes"), "utf8"),
          path,
        );
      }
    }
    const events = JSON.parse(readFileSync(join(dir, "events.json"), "utf8"));
    let results = "";
    for (const event of events) {
      const payload = event.payload === null ? undefined : event.payload;
      results += `${await host.dispatch(event.render, event.event, event.occurrence, payload)}\n`;
    }
    assert.equal(results, readFileSync(join(dir, "events.out"), "utf8"), dir);
  }
});
