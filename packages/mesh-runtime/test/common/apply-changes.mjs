// A reference applier for the changes `diff` makes and `updateChanges` takes,
// over plain data: what the edits mean (docs/manual/runtime.md, "Changes"),
// for tests that check the two agree. It is the specification in code, not the
// runtime's implementation, which is Rust.

/** `snapshot` with `changes` applied in order; `snapshot` is untouched. */
export function applyChanges(snapshot, changes) {
  const out = structuredClone(snapshot);
  for (const change of changes) {
    const { path } = change;
    let parent = out;
    for (const step of path.slice(0, -1)) {
      parent = parent[step];
    }
    const last = path[path.length - 1];
    if (change.op === "set") {
      parent[last] = structuredClone(change.value);
    } else if (change.op === "insert") {
      parent.splice(last, 0, structuredClone(change.value));
    } else if (Array.isArray(parent)) {
      parent.splice(last, 1);
    } else {
      delete parent[last];
    }
  }
  return out;
}

/** `value` with every `undefined` field removed, as a snapshot treats one: absent. */
export function clean(value) {
  if (Array.isArray(value)) return value.map(clean);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, field]) => field !== undefined)
        .map(([name, field]) => [name, clean(field)]),
    );
  }
  return value;
}
