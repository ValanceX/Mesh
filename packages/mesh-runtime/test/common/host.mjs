// The test host (D10), in Node: the Rust tests' host
// (`crates/mesh-runtime/tests/common/host.rs`). It supplies snapshots,
// keeps every render, relays each event to dispatch with the render it
// names, and records what dispatch gives.
import { dispatch, render } from "../../dist/index.js";
import { canonical } from "./json.mjs";

export class Host {
  constructor({ model, root, templates }) {
    this.model = model;
    this.program = { root, templates };
    /** Every render, in order: the host keeps them all. */
    this.renders = [];
  }

  /** Renders a snapshot, keeping the render. Throws with the diagnostics if it fails. */
  async render(snapshot) {
    const result = await render({ program: this.program, model: this.model, snapshot });
    if (result.diagnostics) {
      throw new Error(JSON.stringify(result.diagnostics));
    }
    this.renders.push(result.render);
    return result.render;
  }

  /**
   * Dispatches `event` of the `occurrence`th node binding it (document
   * order) in render `index`, with that render. The result as the
   * document a host would record: the intent, or the diagnostics.
   */
  async dispatch(index, event, occurrence, payload) {
    const kept = this.renders[index];
    const handlers = [];
    collect(kept.tree.root, event, handlers);
    const result = await dispatch(kept, handlers[occurrence], payload);
    return canonical(result.intent ?? result.diagnostics);
  }
}

function collect(node, event, out) {
  if (event in node.events) out.push(node.events[event]);
  for (const child of node.children) {
    if (child.type === "node") collect(child, event, out);
  }
}
