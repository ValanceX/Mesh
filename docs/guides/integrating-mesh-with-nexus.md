# Integrating MESH with NEXUS

This guide is for whoever builds the adapter between NEXUS and MESH: the code that feeds a NEXUS application's state to MESH templates, and turns what the user does back into NEXUS commands. In MESH's terms, that adapter is a **host**. The adapter lives in the NEXUS repository, not in MESH. MESH gives it `@valancex/mesh-compiler`, to compile templates, and `@valancex/mesh-runtime`, to render and dispatch them. This guide says what the runtime expects of a host, and ends with a small working host.

**You'll need:** Node 22 or 24, or a browser with WebAssembly.

```console
$ npm install @valancex/mesh-compiler @valancex/mesh-runtime
```

## What a host does

MESH never reads your state, and never runs a command. Between the two, a host:

1. **Compiles** each component's MPRX against the component manifest, once, when the application is built: `compile()` returns the component's template, or diagnostics. A **program** is the templates, and the name of the root component.
2. **Renders** the program against a **snapshot**: the root template's scope values, from NEXUS's selectors. `render()` returns a **render**, whose `tree` a renderer draws, or diagnostics.
3. **Keeps** each render while its tree is on screen.
4. **Dispatches** an event the renderer reports, a handler identifier and a payload, with the render whose tree the renderer drew. `dispatch()` returns a **command intent**, or diagnostics.
5. **Maps** the intent to a NEXUS command, and runs it. State changes, and the host renders again.

The runtime computes everything in between: every expression, every text, every decision about what's shown. The host only moves values. The reference for all of this is the [runtime manual](../manual/runtime.md).

## Pair each event with its render

**A renderer reports an event against the tree it drew, which may no longer be the newest.** State can change, and the host can render again, between the user's click and the host handling it. If the host dispatches with the newer render, the intent carries values the user never saw: they clicked "Ada Lovelace", and the command gets whoever is in that place now.

So a host keeps each render until the renderer has drawn a newer one, and dispatches each event with the render that was on screen when it fired. Only the host can know which that was, so this is the host's obligation, not the runtime's. `dispatch()` evaluates against the render's own snapshot, taken when `render()` was called, never against the host's current state.

Every render of one program has the same handler identifiers, so a mispaired dispatch doesn't fail: it silently uses the wrong values. The example below shows the difference.

## When the program changes

Keys and handler identifiers are stable across renders of one program, and change when the program does: a template recompiled, added or removed. A renderer reconciles a new tree against the one it drew by key, so **tell the renderer when a tree comes from a different program**, and it draws that tree afresh. A render of an earlier program can still be dispatched, and gives that program's intent.

## Shaping values to the manifest

- **The snapshot is open; records are exact.** A snapshot may hold names the root template doesn't declare, and the runtime ignores them. But a record, anywhere in a value, may hold only the fields its type declares. A NEXUS selector that returns a user with more fields than the manifest's `User` is refused, with `runtime-unknown-field` at the extra field. Shape the value to the manifest in the adapter.
- **Absence is a missing property.** A missing property, and one that is `undefined`, are both absent, and absence is allowed only where the type is optional. `null` is a value, not absence. An array may not hold `undefined` or a hole.
- **Payloads are checked too.** A payload must fit its event's payload type in the manifest (the slice's avatar click carries a `Press`, `{ x, y }`), and an event with no payload type takes none. Pass the renderer's payload through as it is; a mismatch is a diagnostic.
- **Only plain data crosses:** `null`, booleans, finite numbers, strings, arrays and plain objects. A `Date`, a `Map`, a class instance, NaN or a string with an unpaired surrogate is reported, never converted. Turn an `Option` into a present value or a missing property, and a `Date` into a string or a number, before rendering.

## Mapping intents to commands

An intent names a command by the component whose template declares it and the command's name, `user-card`'s `selectUser`, and holds its evaluated arguments in parameter order: `{ value }` for a present argument, or `{ absent: true }`. The command's meaning belongs to NEXUS: which NEXUS command it is (`"users.select"`), and what its input looks like (`{ userId }`), is a table the adapter owns. MESH has no NEXUS names in it.

## Diagnostics

`render()` and `dispatch()` return diagnostics instead of a result when something is wrong: the program, the manifest, the snapshot, the payload or the handler identifier. They're errors in the host's inputs or in the program. They're for the developer, and never something to show an end user as it is. Log them with their `code` and `location`, and treat them as bugs. They never throw: the promises reject only for arguments of the wrong JavaScript type or a render the package didn't make (`TypeError`), a module of another version (`MeshVersionError`), or a failure of the runtime itself (`MeshInternalError`).

## A host, end to end

This host runs the slice, `examples/slice/` in the MESH repository: a `users` page that shows two `user-card`s, whose avatar's click calls `selectUser(user)`. A plain object stands in for NEXUS's state. The user clicks the first avatar; before the host handles the click, the state changes and the host renders again; the host dispatches the click with the render that was on screen, and maps the intent to NEXUS's `users.select`.

```js
import { readFileSync } from "node:fs";
import { compile } from "@valancex/mesh-compiler";
import { dispatch, render } from "@valancex/mesh-runtime";

const read = (name) => readFileSync(`examples/slice/${name}`, "utf8");
const model = read("components.json");

// Compile each template once, when the application is built.
const templates = [];
for (const component of ["users", "user-card"]) {
  const result = await compile({
    source: read(`${component}.mprx`),
    path: `${component}.mprx`,
    model: { manifest: model, path: "components.json", component },
  });
  if (!result.template) throw new Error(JSON.stringify(result.diagnostics));
  templates.push(JSON.stringify(result.template));
}
const program = { root: "users", templates };

// A stand-in for NEXUS's state, already shaped to the manifest.
let state = JSON.parse(read("snapshots/first.json"));

async function renderState() {
  const result = await render({ program, model, snapshot: state });
  if (result.diagnostics) throw new Error(JSON.stringify(result.diagnostics));
  return result.render;
}

// The adapter's own table: a MESH command to a NEXUS command and its input.
const commands = {
  "user-card/selectUser": ([user]) => ["users.select", { userId: user.value.id }],
  "users/refresh": () => ["users.refresh", {}],
};

// The renderer draws this render's tree, and the host keeps the render.
const drawn = await renderState();

// The user clicks the first avatar. The renderer reports the handler
// identifier from the tree it drew, and the payload.
const avatar = drawn.tree.root.children[0].children[0];
const event = { handler: avatar.events.click, payload: { x: 12, y: 34 } };

// Before the host handles the click, the state changes, and it renders again.
state = JSON.parse(read("snapshots/second.json"));
const newer = await renderState();
console.log(`on screen: ${drawn.tree.root.children[0].props.title}`);
console.log(`newest:    ${newer.tree.root.children[0].props.title}`);

// Dispatch with the render that was on screen when the event fired.
const result = await dispatch(drawn, event.handler, event.payload);
if (result.diagnostics) throw new Error(JSON.stringify(result.diagnostics));
const { command, arguments: args } = result.intent;
console.log(`intent:    ${command.component}'s ${command.name}(${args[0].value.name})`);

const [name, input] = commands[`${command.component}/${command.name}`](args);
console.log(`command:   ${name} ${JSON.stringify(input)}`);
```

It prints:

```text
on screen: Ada Lovelace
newest:    Ada King
intent:    user-card's selectUser(Ada Lovelace)
command:   users.select {"userId":"u1"}
```

The intent carries "Ada Lovelace", the user who was on screen. Dispatched with `newer`, it would have carried "Ada King".

## What MESH doesn't do

- **Run commands, or hold state.** Those are NEXUS's.
- **Choose which render to dispatch with.** The host pairs each event with its render.
- **Draw anything.** A renderer does: see [Rendering MESH output](./rendering-mesh-output.md).
- **Map commands.** The mapping from an intent to a NEXUS command is the adapter's table.
