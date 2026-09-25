// JSON as the Rust side of the tests writes it (serde_json's `Value`):
// compact, with object keys in code-point order. Test code only: the
// package itself never formats a value (I11).

/** Compares two strings by code point, as Rust's `str` ordering does. */
export function byCodePoint(a, b) {
  const x = Array.from(a);
  const y = Array.from(b);
  for (let index = 0; index < Math.min(x.length, y.length); index++) {
    const difference = x[index].codePointAt(0) - y[index].codePointAt(0);
    if (difference !== 0) return difference;
  }
  return x.length - y.length;
}

/** `value` as compact JSON with sorted keys. */
export function canonical(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonical).join(",")}]`;
  }
  if (typeof value === "object" && value !== null) {
    const keys = Object.keys(value).sort(byCodePoint);
    return `{${keys.map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}
