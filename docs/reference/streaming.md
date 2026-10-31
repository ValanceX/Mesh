# Streaming: checking a source while it arrives

> **TL;DR:** Check MPRX while a model writes it: a stream session reports a mistake a few bytes after it is written and previews what is safe to draw.

---

A model writes MPRX a few tokens at a time. A batch check of half a file reports a syntax error and nothing else (275 of the 276 prefixes of a small page, in the measurement behind this design), so a wrong prop written early is invisible until the file ends and the model hears nothing meanwhile. A **stream session** checks what has arrived, reports a mistake a few bytes after it is written, and gives the host a checked template to draw while the rest comes.

```js
import { openSession } from "@valancex/mesh-compiler";

const session = await openSession({ manifest, component: "page" });

for await (const chunk of model) {
  const step = await session.push(chunk);

  step.diagnostics;   // the mistakes this chunk added, each reported once
  step.preview;       // { text, template } while nothing is wrong, else absent
  step.pending;       // what is open: [{ kind: "element", name: "card", start: 8 }, ...]
}

const document = await session.close(); // exactly what check() gives for the whole text
```

**Provisional.** It is for a host that streams generated UI; nothing in an ordinary application loads it. In a browser call `init` first, as for any check.

## What is checked: the repaired prefix

After each chunk the module cuts the text back to its **last complete token** and closes what is open, then checks that. The repair is the module's, not the host's, and the model never sees the closers it added. A token is a complete start tag, end tag, `{expression}`, block header (`{#if …}`, `{:else if …}`, `{#each …}`, `{#snippet …}`), block closer, `{@render …}` or run of text.

- **Nothing is kept that was cut inside a start tag, an attribute string, an expression or a block header.** Closing a cut attribute string checks clean with a different value in 39 of 40 prefixes, so drawing it would show wrong data.
- **A trailing run of text is kept**, growing, inside anything already open. A chunk that ends between the two halves of a surrogate pair is held back until the other half comes.
- **An open `{#each}` is withheld** until its `{/each}` arrives. A repeat whose body is more than one element is not a prefix of its final tree (the body becomes a fragment when it closes), so showing it early would show rows that move.
- **A start tag still being written is checked as far as it has got** for what is already final about it: a name that is not a component, an attribute or event the component lacks, a reference to nothing in scope, a value of the wrong type. That is why a wrong prop is heard when its attribute is complete, not when the tag is. A required prop that has not arrived yet is not reported: it may be the next attribute.

A mistake is reported once, at the first prefix that holds it (`step.diagnostics`); `step.standing` is everything standing now. Match on `code`, never on `message`, and fix each diagnostic once, as for a finished file. The positions are those of the text that arrived.

## The preview

`step.preview` is the checked repaired prefix and its `template-v1`, **only while no error stands** (a warning does not stop it). Draw it as a **fresh render**: a repaired prefix is a different program from the next one, and node identity includes the program, so no node keeps its key between steps and `update` cannot carry one preview to the next. What it holds is a prefix of what the whole text will hold, except that a trailing text run may be shorter (tested by rendering every prefix of every page in `examples/conformance/generated/` in three states of the data and comparing it with the final tree).

Nothing unchecked is drawn: an unknown component, an unknown prop or a reference to nothing is a diagnostic, and a diagnostic stands for every later prefix, so there is no preview from that point on. A host that shows something meanwhile shows the last preview or text.

## The law, the limits and the budget

- **`close()` is the batch check of the whole text**, whatever was pushed and however it was cut: the session keeps only the text and the set of diagnostics it has reported. Property-tested over 100,000 random prefixes and mutations in Rust (one to five chunks each) and 3,000 through WebAssembly.
- **The input limits** (a source of 64 KB, 5,000 elements, 64 attributes on an element, 128 levels) hold in a session: past one, the diagnostic (`input-too-large`, `too-many-elements`, `too-many-attributes`, `nesting-too-deep`) and no preview. Measured through WebAssembly: 4,500 siblings (63 KB) in one push take about 350 ms, 5.5 ms per kilobyte at worst; a source over the limit is refused in under 2 ms.
- **A push has a budget of 50 milliseconds of checker time** (`budgetMs`). A push over it ends the session with `ended: { code: "check-budget-exceeded" }`; later pushes do no work and return the same, and `close()` still gives the whole check. A typical page costs under a millisecond per push (the mean over `nested.mprx` fed one character at a time is 0.75 ms, the worst 2.8 ms). The check is of the whole repaired prefix on each push, not incremental: the cost of a stream of n pushes grows with n times the length, bounded by the budget, which ends a sibling-heavy stream at about 600 siblings. A trip is a fact about the machine, so a replay of a session must log it as an input.

## Where it is, and what it is not

The repair and the check are in `mesh_compiler::stream` (`repair`, `step`, `Session`), and `mesh_wasm` exports one stateless function (`mesh_stream`) over them; the JavaScript session keeps the text and the reported set. Rendering a preview, the profile that says what a model may use, the correction loop and the log of a generation session are the host's (`@valancex/valance/generate`); nothing here renders, calls a model or runs a command.

## Next steps

* [Using MESH from JavaScript](../how-to/using-mesh-from-javascript.md).
