# The slice

The smallest whole MESH program: a `users` page whose template uses the composite `user-card` twice. It's what v0.5's Definition of Done calls "the slice", and the tests run it natively (`crates/mesh-runtime/tests/slice.rs`) and in Node (`packages/mesh-runtime/test/slice.test.mjs`).

- `components.json` is its manifest. `page`, `text`, `avatar` and `button` are primitives, which a renderer draws. `user-card` has a template in the program, so it's a composite: its props `user` and `compact` bind its scope, and its avatar's click invokes its own command, `selectUser`.
- `users.mprx` and `user-card.mprx` are the two templates. The tests compile them; no compiled template is committed.
- `snapshots/first.json` and `snapshots/second.json` are the host's values: the second changes the first user's name and avatar.
- `expected/` holds what the runtime gives, reviewed by hand:
  - `first.tree.json`: the render tree of the first snapshot, with only primitives in it;
  - `first.html`: the reference renderer's printing of it;
  - `select-first.intent.json`: the intent from the first avatar's click, `user-card`'s `selectUser` with the first user;
  - `first-to-second.changes`: what changes, key by key, when the second snapshot is rendered.

The keys and handler identifiers in `first.tree.json` are real, and change whenever either template does. The files under `examples/slice/` aren't part of the check corpus that `examples/*.mprx` and `examples/fixtures/` are.
