# Getting started with MESH v0.1

This guide covers installing the `mesh` command, writing your first MPRX file, and reading the diagnostics MESH prints. It takes about ten minutes.

**You'll need:** Rust 1.91 or newer (install it from [rustup.rs](https://rustup.rs)) and `git`. Inside the repo, rustup picks the exact toolchain named in `rust-toolchain.toml`. You don't need Node.js unless you plan to work on the Tree-sitter grammar.

## 1. Install the CLI

MESH v0.1 is distributed as source. Clone the repo and install the `mesh` binary with Cargo:

```console
$ git clone https://github.com/ValanceX/Mesh.git
$ cd Mesh
$ cargo install --path crates/mesh-cli
```

Cargo puts `mesh` in `~/.cargo/bin`, which rustup already added to your `PATH`. Check that it works:

```console
$ mesh --help
The MESH command-line toolchain: parse, check, and (eventually) compile MPRX source files

Usage: mesh <COMMAND>
...
```

> **Just trying it out?** You can skip installing and run the CLI straight from the repo with `cargo run -p mesh-cli -- check <file>`. Every `mesh check` in these docs works the same way.

## 2. Write your first file

Save this as `hello.mprx`:

```xml
<page title="Users">
  <heading>Users</heading>
  <user-card user={user} compact={layout.compact} on.select={selectUser($event)} />
</page>
```

This is a complete MPRX document: a single root element (`page`) with two children. It has three ingredients:

- **Attributes** with a plain string (`title="Users"`) or an expression in braces (`user={user}`).
- **Text** between tags (`Users`).
- An **event binding** (`on.select={...}`). This says: when this `user-card` emits `select`, ask the app to run the `selectUser` command with the event's value. MESH records that request but never runs it. NEXUS, Valance's runtime layer, decides what `selectUser` does.

## 3. Check it

```console
$ mesh check hello.mprx
no errors
```

`mesh check` parses the file, builds its semantic model, and runs the v0.1 structural checks. When nothing is wrong, it prints `no errors` and exits with status `0`.

## 4. Break it on purpose

Change the closing `</heading>` to `</title>` and check again:

```console
$ mesh check hello.mprx
error[mismatched-closing-tag]: mismatched closing tag: opened with "heading", closed with "title"
 --> hello.mprx:2:3
  |
2 |   <heading>Users</title>
  |   ^^^^^^^^^^^^^^^^^^^^^^
```

Each diagnostic is a block in the style of the Rust compiler:

- The first line gives the **severity** (`error` or `warning`), the diagnostic's **code** in brackets, and a message. The [diagnostics reference](../manual/diagnostics.md) explains every code.
- `-->` gives the **location** as `file:line:column`. Lines and columns start at 1.
- The source line is echoed, and carets (`^`) underline the problem.

Errors make `mesh check` exit with status `1` and suppress `no errors`, so it can be used as a CI gate.

Now undo that change and repeat an attribute:

```xml
<heading level="1" level="2">Users</heading>
```

```console
$ mesh check hello.mprx
warning[duplicate-attribute]: duplicate attribute "level": this occurrence is shadowed by a later one
 --> hello.mprx:2:12
  |
2 |   <heading level="1" level="2">Users</heading>
  |            ^^^^^^^^^

no errors
```

A **warning** is printed the same way, but it doesn't fail the check. The output still ends with `no errors`, and the exit status is `0`. Here the later `level="2"` wins, and the warning points at the one that was ignored.

## 5. Look at the examples

The repo ships two canonical v0.1 examples that always check clean:

```console
$ mesh check examples/user-card.mprx
no errors
$ mesh check examples/users-page.mprx
no errors
```

`examples/fixtures/` has a small corpus of deliberately broken files. Each one sits next to a `.stderr` file with the exact output `mesh check` prints for it. Browse it to see every kind of diagnostic v0.1 produces.

## Where next

- [Writing MPRX](./writing-mprx.md): a tour of everything the v0.1 language supports, plus common mistakes.
- [`mesh` CLI manual](../manual/mesh-cli.md): exact output format and exit codes.
- [Diagnostics reference](../manual/diagnostics.md): every message and how to fix it.
- [Embedding the compiler](./embedding-the-compiler.md): calling MESH from your own Rust code.
