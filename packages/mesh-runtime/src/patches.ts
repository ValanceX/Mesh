/**
 * Applying `render-patch-v1` to a render tree, in JavaScript: what turns the
 * tree of a render into the tree of the render `update` made from it, so that
 * the module returns what changed and not the whole tree.
 *
 * It does what a renderer does (docs/manual/runtime.md, "Update"), over
 * immutable data: the result shares every part the patches don't reach with
 * the tree it came from, and every part is frozen. It trusts the patches:
 * they come from the runtime's own `update`, which checks the law (applying
 * them gives exactly the tree a full render gives), and the package's tests
 * check it again from here.
 *
 * This is the one place the package reads a tree, and it is an exception to
 * I11 that `test/no-evaluation.test.mjs` names: it does what a renderer
 * does with a patch list, which is to move parts by key. It reads no value,
 * makes no text, and converts, defaults and judges nothing; a number, a
 * string or a record in a patch goes into the tree exactly as the runtime
 * wrote it. Without it the module would send the whole tree on every update,
 * and an update would cost the size of the tree again.
 */

import type { RenderNode, RenderPatches, RenderTree, TextRun } from "./types.js";

type Part = RenderNode | TextRun;

/** Every part of a tree by key, with its parent's key. */
interface Index {
  readonly parts: ReadonlyMap<string, Part>;
  readonly parents: ReadonlyMap<string, string | undefined>;
}

/** The index of a tree, made when it is first updated and kept for as long as the tree is. */
const indexes = new WeakMap<RenderTree, Index>();

function indexOf(tree: RenderTree): Index {
  let index = indexes.get(tree);
  if (index === undefined) {
    const parts = new Map<string, Part>();
    const parents = new Map<string, string | undefined>();
    const walk = (part: Part, parent: string | undefined): void => {
      parts.set(part.key, part);
      parents.set(part.key, parent);
      if (part.type === "node") {
        for (const child of part.children) {
          walk(child, part.key);
        }
      }
    };
    walk(tree.root, undefined);
    index = { parts, parents };
    indexes.set(tree, index);
  }
  return index;
}

/** `record` without `name`, or with `name` set to `value`; frozen. */
function withEntry<V>(record: Readonly<Record<string, V>> | undefined, name: string, value: V | undefined): Readonly<Record<string, V>> {
  const next: Record<string, V> = { ...record };
  if (value === undefined) {
    delete next[name];
  } else {
    next[name] = value;
  }
  return Object.freeze(next);
}

/**
 * The tree `patches` make of `tree`. The tree is untouched; the result shares
 * every part the patches don't reach with it.
 */
export function applyPatches(tree: RenderTree, patches: RenderPatches): RenderTree {
  const list = patches.patches;
  if (list.length === 0) {
    return tree;
  }
  const first = list[0]!;
  if (first.op === "replace") {
    return first.tree;
  }

  const index = indexOf(tree);
  /** Parts as the patches leave them, where they differ from the tree's. */
  const parts = new Map<string, Part>();
  const parents = new Map<string, string | undefined>();
  /** A node's children's keys, once a patch has changed which they are or their order. */
  const kids = new Map<string, string[]>();
  /** The parts to rebuild: every part a patch changed, and every ancestor of one. */
  const dirty = new Set<string>();

  const get = (key: string): Part => parts.get(key) ?? index.parts.get(key)!;
  const parentOf = (key: string): string | undefined => (parents.has(key) ? parents.get(key) : index.parents.get(key));
  const keysUnder = (key: string): string[] => {
    let found = kids.get(key);
    if (found === undefined) {
      found = (get(key) as RenderNode).children.map((child) => child.key);
      kids.set(key, found);
    }
    return found;
  };
  /** An ancestor of a dirty part is dirty already, so the climb stops at the first that is. */
  const touch = (start: string | undefined): void => {
    for (let key = start; key !== undefined && !dirty.has(key); key = parentOf(key)) {
      dirty.add(key);
    }
  };
  const at = (siblings: readonly string[], before: string | undefined): number => (before === undefined ? siblings.length : siblings.indexOf(before));

  for (const patch of list) {
    switch (patch.op) {
      case "setProp": {
        const node = get(patch.key) as RenderNode;
        const text = patch.propText;
        const next: Record<string, unknown> = { ...node, props: withEntry(node.props, patch.prop, patch.value) };
        const propText = withEntry(node.propText, patch.prop, text);
        if (Object.keys(propText).length > 0) {
          next.propText = propText;
        } else {
          delete next.propText;
        }
        parts.set(patch.key, next as unknown as RenderNode);
        touch(patch.key);
        break;
      }
      case "removeProp": {
        const node = get(patch.key) as RenderNode;
        const next: Record<string, unknown> = { ...node, props: withEntry(node.props, patch.prop, undefined) };
        const propText = withEntry(node.propText, patch.prop, undefined);
        if (Object.keys(propText).length > 0) {
          next.propText = propText;
        } else {
          delete next.propText;
        }
        parts.set(patch.key, next as unknown as RenderNode);
        touch(patch.key);
        break;
      }
      case "setText": {
        parts.set(patch.key, { ...(get(patch.key) as TextRun), text: patch.text });
        touch(patch.key);
        break;
      }
      case "insert": {
        const register = (part: Part, parent: string): void => {
          parts.set(part.key, part);
          parents.set(part.key, parent);
          if (part.type === "node") {
            for (const child of part.children) {
              register(child, part.key);
            }
          }
        };
        register(patch.node, patch.parent);
        const siblings = keysUnder(patch.parent);
        siblings.splice(at(siblings, patch.before), 0, patch.node.key);
        touch(patch.parent);
        break;
      }
      case "remove": {
        const parent = parentOf(patch.key)!;
        const siblings = keysUnder(parent);
        siblings.splice(siblings.indexOf(patch.key), 1);
        touch(parent);
        break;
      }
      case "move": {
        const parent = parentOf(patch.key)!;
        const siblings = keysUnder(parent);
        siblings.splice(siblings.indexOf(patch.key), 1);
        siblings.splice(at(siblings, patch.before), 0, patch.key);
        touch(parent);
        break;
      }
      case "replace":
        return patch.tree;
    }
  }

  const build = (key: string): Part => {
    const part = get(key);
    if (part.type === "text" || !dirty.has(key)) {
      return part;
    }
    const keys = kids.get(key);
    const children = keys === undefined ? part.children.map((child) => build(child.key)) : keys.map(build);
    return Object.freeze({ ...part, children: Object.freeze(children) }) as RenderNode;
  };
  // A part a patch replaced is a new object, and so is every ancestor: frozen here, once.
  for (const key of dirty) {
    const part = parts.get(key);
    if (part !== undefined && !Object.isFrozen(part)) {
      Object.freeze(part);
    }
  }
  return Object.freeze({ ...tree, root: build(tree.root.key) as RenderNode });
}
