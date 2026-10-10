// A program with a conditional, a keyed repeat and an optional prop: the same
// shape as the Rust runtime's law tests, and a deterministic generator of random
// states for it. Shared by the update and changes tests.
import { template } from "../common.mjs";

// A program with a conditional, a keyed repeat and an optional prop: the same shape as the Rust law test.
export const LIST_MODEL = JSON.stringify({
  version: 1,
  types: {},
  components: {
    page: {
      props: {
        title: { type: { kind: "string" }, required: true },
        count: { type: { kind: "number" }, required: true },
        tag: { type: { kind: "optional", type: { kind: "string" } }, required: false },
      },
      events: { tap: {} },
      commands: {},
      scope: {},
    },
    note: { props: {}, events: {}, commands: {}, scope: {} },
    row: {
      props: { label: { type: { kind: "string" }, required: true }, done: { type: { kind: "boolean" }, required: true } },
      events: { tap: {} },
      commands: {},
      scope: {},
    },
    "mesh-if": { props: { when: { type: { kind: "boolean" }, required: true } }, events: {}, commands: {}, scope: {} },
    "mesh-each": {
      props: {
        items: { type: { kind: "list", element: { kind: "any" } }, required: true },
        as: { type: { kind: "string" }, required: true },
        key: { type: { kind: "any" }, required: true },
      },
      events: {},
      commands: {},
      scope: {},
    },
    view: {
      props: {},
      events: {},
      commands: { pick: { parameters: [{ name: "id", type: { kind: "number" } }] } },
      scope: {
        title: { kind: "string" },
        count: { kind: "number" },
        flag: { kind: "boolean" },
        tag: { kind: "optional", type: { kind: "string" } },
        items: {
          kind: "list",
          element: {
            kind: "record",
            fields: {
              id: { type: { kind: "number" }, required: true },
              label: { type: { kind: "string" }, required: true },
              done: { type: { kind: "boolean" }, required: true },
            },
          },
        },
      },
    },
  },
});
const LIST_VIEW =
  '<page title={title} count={count + 1} tag={tag} on.tap={pick(count)}><note>{title} has {count}</note><mesh-if when={flag}><note>on</note><note>off</note></mesh-if><mesh-each items={items} as="item" key={item.id}><row label={item.label} done={item.done} on.tap={pick(item.id)}>{item.label}</row></mesh-each></page>';
export const listProgram = { root: "view", templates: [await template("view", LIST_VIEW, LIST_MODEL)] };

/** A small deterministic generator, so the test needs no dependency. */
export function generator(seed) {
  let state = seed;
  const below = (n) => {
    state = (Math.imul(state, 1103515245) + 12345) >>> 0;
    return (state >>> 8) % n;
  };
  return below;
}

export function mutate(state, below) {
  const next = structuredClone(state);
  for (let i = 0; i <= below(3); i++) {
    switch (below(9)) {
      case 0: next.title = `t${below(4)}`; break;
      case 1: next.count = below(5) - 2; break;
      case 2: next.flag = below(2) === 1; break;
      case 3: if (below(2)) next.tag = `g${below(3)}`; else delete next.tag; break;
      case 4: {
        const ids = [0, 1, 2, 3, 4, 5, 6, 7];
        next.items = Array.from({ length: below(6) }, () => ({ id: ids.splice(below(ids.length), 1)[0], label: `l${below(3)}`, done: below(2) === 1 }));
        break;
      }
      case 5: if (next.items.length) { const it = next.items[below(next.items.length)]; it.label = `l${below(3)}`; it.done = below(2) === 1; } break;
      case 6: if (next.items.length > 1) { const a = below(next.items.length); const b = below(next.items.length); [next.items[a], next.items[b]] = [next.items[b], next.items[a]]; } break;
      case 7: {
        const id = [0, 1, 2, 3, 4, 5, 6, 7].find((candidate) => !next.items.some((x) => x.id === candidate));
        if (next.items.length && below(2)) next.items.splice(below(next.items.length), 1);
        else if (id !== undefined) next.items.splice(below(next.items.length + 1), 0, { id, label: "new", done: false });
        break;
      }
    }
  }
  return next;
}

