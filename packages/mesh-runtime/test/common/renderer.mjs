// The reference renderer (D10), in Node: the Rust tests' renderer
// (`crates/mesh-runtime/tests/common/renderer.rs`), with the same printed
// form, so both produce the same committed files. It draws what it's
// given and computes nothing: prop values print as the JSON they are,
// and text runs as their strings.
import { byCodePoint, canonical } from "./json.mjs";

/** The tree as indented, HTML-like text; keys aren't printed. */
export function print(tree) {
  const out = [];
  node(tree.root, 0, out);
  return out.join("");
}

function node(n, depth, out) {
  const indent = "  ".repeat(depth);
  let tag = `${indent}<${n.component}`;
  for (const name of Object.keys(n.props).sort(byCodePoint)) {
    tag += ` ${name}=${canonical(n.props[name])}`;
  }
  for (const event of Object.keys(n.events).sort(byCodePoint)) {
    tag += ` on.${event}`;
  }
  if (n.children.length === 0) {
    out.push(`${tag} />\n`);
    return;
  }
  out.push(`${tag}>\n`);
  for (const child of n.children) {
    if (child.type === "node") {
      node(child, depth + 1, out);
    } else {
      out.push(`${indent}  ${JSON.stringify(child.text)}\n`);
    }
  }
  out.push(`${indent}</${n.component}>\n`);
}

/** Every node and text run, by key: where it is, and its props or text. */
function parts(n, at, out) {
  out.set(n.key, { kind: "node", at, props: n.props });
  n.children.forEach((child, index) => {
    if (child.type === "node") {
      parts(child, `${at}/${child.component}[${index}]`, out);
    } else {
      out.set(child.key, { kind: "text", at: `${at}/text[${index}]`, text: child.text });
    }
  });
}

/**
 * Every prop and text run whose value changed from `old` to `next`,
 * matched by key, one per line. Throws if the trees' keys differ: two
 * renders of one program always have the same keys.
 */
export function compare(old, next) {
  const before = new Map();
  const after = new Map();
  parts(old.root, old.root.component, before);
  parts(next.root, next.root.component, after);
  const keys = (map) => [...map.keys()].sort(byCodePoint);
  if (canonical(keys(before)) !== canonical(keys(after))) {
    throw new Error("two renders of one program have the same keys");
  }
  const changes = [];
  for (const key of keys(after)) {
    const a = before.get(key);
    const b = after.get(key);
    if (a.kind !== b.kind) throw new Error("a key changed kind");
    if (b.kind === "node") {
      const names = [...new Set([...Object.keys(a.props), ...Object.keys(b.props)])].sort(byCodePoint);
      for (const name of names) {
        const show = (props) => (name in props ? canonical(props[name]) : "(absent)");
        if (show(a.props) !== show(b.props)) {
          changes.push([b.at, `${b.at} ${name}: ${show(a.props)} -> ${show(b.props)}`]);
        }
      }
    } else if (a.text !== b.text) {
      changes.push([b.at, `${b.at}: ${JSON.stringify(a.text)} -> ${JSON.stringify(b.text)}`]);
    }
  }
  changes.sort((x, y) => byCodePoint(x[0], y[0]) || byCodePoint(x[1], y[1]));
  return changes.map(([, line]) => `${line}\n`).join("");
}
