/**
 * `diff`: the changes from one snapshot to the next (docs/manual/runtime.md,
 * "Changes"), computed by the host from the two snapshots it has, never from
 * what its code meant to change. A command that mutates the wrong thing, or
 * two things, or nothing, still leaves a previous and a next state, and the
 * difference between *those* is what the runtime is told.
 *
 * This is a host-side utility, on no path of `render`, `update` or `dispatch`:
 * the engine doesn't import it, and the package's review test (I11) names it
 * as the one place, besides the encoder, that reads host values, and checks it
 * makes no text and no number. It judges nothing about a value: it compares
 * two values for sameness, exactly as the boundary does (§9.8.2), and an edit
 * carries the host's own value, as given, for the runtime to validate.
 *
 *  - A number is the same as another if it is the same binary64 value, so `0`
 *    and `-0` differ (a snapshot keeps `-0`), and a string is the same if it is
 *    the same string.
 *  - A record is a plain object, and a missing property and one that is
 *    `undefined` are both absent. Two records are the same if each field is.
 *  - A list is an array. A hole or `undefined` in one is refused by the runtime;
 *    the list is then given whole, to be refused there, not diffed.
 *  - Anything else (a `Map`, a `Date`, a class instance, a function) is given
 *    whole, so the runtime reports it as it would for a whole snapshot.
 *
 * Applying the result to `previous`, in order, gives `next`. It is not a
 * shortest edit script. A list is diffed by its common start and end, and
 * the elements between them one for one, with any extra at the end inserted or
 * removed; a reorder is therefore a run of element edits, and a list that
 * changed throughout is given whole.
 */

import type { Change, PathStep } from "./engine.js";

/** How deep a value may nest, as the runtime's limit (§9.8.5). Past it, a value is given whole. */
const DEEP = 128;

/** Whether `value` is a record: a plain object. */
function plain(value: unknown): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value) as unknown;
  return prototype === Object.prototype || prototype === null;
}

/** The names of `record`'s present fields: a field that is `undefined` is absent. */
function present(record: Record<string, unknown>): string[] {
  return Object.keys(record).filter((field) => record[field] !== undefined);
}

/** Whether two values are the same by the boundary's equality. */
function same(a: unknown, b: unknown, depth: number): boolean {
  if (depth > DEEP) {
    return false;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((element, index) => same(element, b[index], depth + 1));
  }
  if (plain(a) && plain(b)) {
    const names = present(a);
    return names.length === present(b).length && names.every((field) => same(a[field], b[field], depth + 1));
  }
  return Object.is(a, b);
}

function walk(path: PathStep[], a: unknown, b: unknown, depth: number, out: Change[]): void {
  if (same(a, b, depth)) {
    return;
  }
  if (depth <= DEEP && plain(a) && plain(b)) {
    const before = new Set(present(a));
    for (const field of present(a)) {
      if (b[field] === undefined) {
        out.push({ op: "remove", path: [...path, field] });
      }
    }
    for (const field of present(b)) {
      if (before.has(field)) {
        walk([...path, field], a[field], b[field], depth + 1, out);
      } else {
        out.push({ op: "set", path: [...path, field], value: b[field] });
      }
    }
    return;
  }
  if (depth <= DEEP && Array.isArray(a) && Array.isArray(b) && !a.includes(undefined) && !b.includes(undefined) && !hole(a) && !hole(b)) {
    list(path, a, b, depth, out);
    return;
  }
  out.push({ op: "set", path, value: b });
}

/** Whether an array has a hole (an index with no element). */
function hole(list: readonly unknown[]): boolean {
  for (let index = 0; index < list.length; index++) {
    if (!(index in list)) {
      return true;
    }
  }
  return false;
}

/** A list, by its common start and end and the elements between, one for one. */
function list(path: PathStep[], a: readonly unknown[], b: readonly unknown[], depth: number, out: Change[]): void {
  let start = 0;
  while (start < a.length && start < b.length && same(a[start], b[start], depth + 1)) {
    start++;
  }
  let end = 0;
  while (end < a.length - start && end < b.length - start && same(a[a.length - 1 - end], b[b.length - 1 - end], depth + 1)) {
    end++;
  }
  const had = a.length - start - end;
  const has = b.length - start - end;
  const shared = had < has ? had : has;
  for (let offset = 0; offset < shared; offset++) {
    walk([...path, start + offset], a[start + offset], b[start + offset], depth + 1, out);
  }
  // Extra elements of the old list go, each at the same index as the one before it did; extra new ones come in, in order.
  for (let offset = shared; offset < had; offset++) {
    out.push({ op: "remove", path: [...path, start + shared] });
  }
  for (let offset = shared; offset < has; offset++) {
    out.push({ op: "insert", path: [...path, start + offset], value: b[start + offset] });
  }
}

/**
 * The changes that turn the snapshot `previous` into `next`, in order, as
 * `updateChanges` takes them (without a `base`: that is the render's `version`).
 * Both are snapshots, plain objects of scope values.
 */
export function diff(previous: Record<string, unknown>, next: Record<string, unknown>): Change[] {
  const out: Change[] = [];
  for (const scopeName of present(previous)) {
    if (next[scopeName] === undefined) {
      out.push({ op: "remove", path: [scopeName] });
    }
  }
  for (const scopeName of present(next)) {
    if (previous[scopeName] === undefined) {
      out.push({ op: "set", path: [scopeName], value: next[scopeName] });
    } else {
      walk([scopeName], previous[scopeName], next[scopeName], 0, out);
    }
  }
  return out;
}
