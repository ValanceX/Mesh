// What an update costs from JavaScript, across the module's boundary, for a
// keyed list of N items with one changed: the full render, and a chain of
// updates (each releasing the render before it). The least of several runs is
// reported, since a machine's noise only adds time. Needs `npm run build:wasm`
// and `npm run build` first; compiles its one template with the compiler package.
//
//   node scripts/bench-update.mjs [items ...]     (default: 1000 10000)
import { render, update, updateChanges } from "../dist/index.js";
import { compile } from "../../mesh-compiler/dist/index.js";

const MODEL = JSON.stringify({
  version: 1, types: {},
  components: {
    page: { props: {}, events: {}, commands: {}, scope: {} },
    row: { props: { title: { type: { kind: "string" }, required: true } }, events: {}, commands: {}, scope: {} },
    "mesh-each": { props: { items: { type: { kind: "list", element: { kind: "any" } }, required: true }, as: { type: { kind: "string" }, required: true }, key: { type: { kind: "any" }, required: true } }, events: {}, commands: {}, scope: {} },
    view: { props: {}, events: {}, commands: {}, scope: { items: { kind: "list", element: { kind: "record", fields: { id: { type: { kind: "number" }, required: true }, label: { type: { kind: "string" }, required: true } } } } } },
  },
});
const SOURCE = '<page><mesh-each items={items} as="item" key={item.id}><row title={item.label}>{item.label}</row></mesh-each></page>';
const compiled = await compile({ source: SOURCE, path: "view.mprx", model: { manifest: MODEL, path: "m.json", component: "view" } });
const program = { root: "view", templates: [JSON.stringify(compiled.template)] };

const items = (n, changed) => ({ items: Array.from({ length: n }, (_, id) => ({ id, label: id === changed ? "changed" : `item ${id}` })) });
const least = async (runs, work) => {
  let best = Infinity;
  for (let i = 0; i < runs; i++) {
    const started = performance.now();
    await work();
    best = Math.min(best, performance.now() - started);
  }
  return best;
};

for (const n of (process.argv.slice(2).map(Number).filter(Boolean).length ? process.argv.slice(2).map(Number) : [1000, 10000])) {
  const base = items(n);
  const edited = items(n, Math.floor(n / 2));
  const fullMs = await least(10, () => render({ program, model: MODEL, snapshot: edited }));
  let { render: current } = await render({ program, model: MODEL, snapshot: base });
  // A chain, alternating the one changed item, each update releasing the render before it.
  let flip = false;
  const updateMs = await least(15, async () => {
    flip = !flip;
    const next = await update(current, flip ? edited : base);
    if (next.diagnostics) throw new Error(JSON.stringify(next.diagnostics));
    current.release();
    current = next.render;
  });
  current.release();
  // The same chain from the changes form: one `set` of the changed label, the module keeping the snapshot.
  let { render: kept } = await render({ program, model: MODEL, snapshot: base });
  const at = Math.floor(n / 2);
  let state = false;
  const changesMs = await least(15, async () => {
    state = !state;
    const next = await updateChanges(kept, { base: kept.version, changes: [{ op: "set", path: ["items", at, "label"], value: state ? "changed" : `item ${at}` }] });
    if (next.diagnostics) throw new Error(JSON.stringify(next.diagnostics));
    kept.release();
    kept = next.render;
  });
  console.log(`${String(n).padStart(6)} items, one changed: full render ${fullMs.toFixed(1)} ms, update ${updateMs.toFixed(1)} ms, update from changes ${changesMs.toFixed(1)} ms`);
  kept.release();
}
