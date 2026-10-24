/**
 * Checking a source that is still arriving (decisions G1 to G14; Bet 13): a session over one component model. `push` the chunks a model writes, as they come; each
 * answer says what is new that is wrong, what is open, and, while nothing is wrong, the part that has arrived as a checked template to draw. `close` when they end.
 *
 * The check is the compiler's, on the **repaired prefix**: the text cut back to its last complete token, with what is open closed (and, for a start tag still being
 * written, what is already wrong with the attributes that have arrived). The repair is done in the module, not here, and the model never sees the closers it adds.
 * Nothing is drawn that was cut inside a start tag, an attribute string, an expression or a block header, and an open `{#each}` is withheld until it closes.
 *
 * The law, tested over 100,000 random prefixes and mutations: `close()` returns exactly what {@link check} returns for the whole text, however it was cut.
 */

import type { Diagnostic, DiagnosticsDocument } from "./document.js";
import { MeshUsageError, check, stream } from "./engine.js";
import type { StreamDocument } from "./engine.js";
import type { Template } from "./template.js";

/** What a session needs to open. */
export interface SessionInput {
  /** The manifest's text: the model a generated source is checked against. For a model, this is the profile (see `@valancex/valance/generate`). */
  manifest: string;
  /** The component whose template the generated source is. */
  component: string;
  /** An opaque identifier for the source, used only to name it in diagnostics. Default `"generated.mprx"`. */
  path?: string;
  /** An opaque identifier for the manifest. Default `"components.json"`. */
  manifestPath?: string;
  /**
   * The milliseconds of checker time one `push` may take (decision G5). A push over it ends the session with `check-budget-exceeded`: the host falls back to the
   * last preview or to text. A trip is a fact about the machine, so a replay of a session logs it as an input and never recomputes it. Default 50.
   */
  budgetMs?: number;
}

/** One thing that is open in the source that arrived. */
export interface Pending {
  kind: "element" | "if" | "each" | "snippet";
  /** An element's tag. */
  name?: string;
  /** The UTF-8 byte where it opened. */
  start: number;
}

/** What one `push` found. */
export interface StreamStep {
  /** The diagnostics this push added: a mistake is reported once, at the first prefix that holds it. Match on `code`. */
  diagnostics: Diagnostic[];
  /** Every diagnostic standing against the text so far. */
  standing: Diagnostic[];
  /**
   * The part that has arrived, checked and compiled, to draw as a fresh render (a repaired prefix is a different program from the next one, so no node keeps its identity
   * between steps). Absent while an error stands, and while nothing is complete.
   */
  preview?: { text: string; template: Template };
  /** The source made whole: the text that was checked, and how many UTF-8 bytes of the source it keeps. */
  repaired: { text: string; cut: number };
  /** What is open, outermost first (an open `{#each}` included): a host may show a placeholder for it. */
  pending: Pending[];
  /** Whether an open `{#each}` was withheld from `preview`. */
  withheld: boolean;
  /** Present when the session ended early; pushing again returns this and does no work. */
  ended?: { code: "check-budget-exceeded"; elapsedMs: number };
}

/** A stream session. */
export interface Session {
  /** Adds a chunk and checks everything that has arrived. A chunk ending in half of a surrogate pair is held until the other half comes. */
  push(chunk: string): Promise<StreamStep>;
  /** Ends the session: the check of the whole text, exactly what `check` gives for it. Pushing after it is a usage error. */
  close(): Promise<DiagnosticsDocument>;
  /** Everything pushed so far (without a trailing half of a surrogate pair). */
  readonly text: string;
}

const key = (diagnostic: Diagnostic): string => `${diagnostic.code}|${diagnostic.span.start.byte}|${diagnostic.span.end.byte}`;

/** Whether `text` ends in the first half of a surrogate pair. */
const endsInHighSurrogate = (text: string): boolean => {
  const last = text.charCodeAt(text.length - 1);

  return last >= 0xd800 && last <= 0xdbff;
};

/**
 * Opens a session. It rejects with a `MeshUsageError` (`invalid-argument`) if an argument is of the wrong type or the manifest does not load (the diagnostics of the
 * manifest are the error's `cause`), so a profile that is wrong is found before a model is asked anything.
 */
export async function openSession(input: SessionInput): Promise<Session> {
  if (typeof input !== "object" || input === null || typeof input.manifest !== "string" || typeof input.component !== "string") {
    throw new MeshUsageError("openSession() takes an object: { manifest, component, path?, manifestPath?, budgetMs? }");
  }

  const path = input.path ?? "generated.mprx";
  const model = { manifest: input.manifest, path: input.manifestPath ?? "components.json", component: input.component };
  const budgetMs = input.budgetMs ?? 50;
  let text = "";
  let carry = "";
  let closed = false;
  let ended: StreamStep["ended"];
  let last: StreamStep | undefined;
  const reported = new Set<string>();

  const opened = (await stream({ source: "", path, model })).result;

  if ("manifest" in opened) {
    throw new MeshUsageError(`the manifest does not load: ${opened.manifest.diagnostics[0]?.message ?? "it has errors"}`, "invalid-argument", { cause: opened.manifest });
  }

  return {
    get text() {
      return text;
    },
    async push(chunk) {
      if (typeof chunk !== "string") {
        throw new MeshUsageError("push() takes a string");
      }

      if (closed) {
        throw new MeshUsageError("this session is closed");
      }

      const pushed = carry + chunk;

      carry = endsInHighSurrogate(pushed) ? pushed.slice(-1) : "";
      text += carry === "" ? pushed : pushed.slice(0, -1);

      if (ended !== undefined && last !== undefined) {
        return { ...last, diagnostics: [], ended };
      }

      const { result, elapsedMs } = await stream({ source: text, path, model });

      if ("manifest" in result) {
        throw new MeshUsageError("the manifest does not load", "invalid-argument", { cause: result.manifest });
      }

      const document: StreamDocument = result;
      const standing = document.diagnostics.diagnostics;
      const fresh = standing.filter((diagnostic) => !reported.has(key(diagnostic)));

      fresh.forEach((diagnostic) => reported.add(key(diagnostic)));

      const step: StreamStep = {
        diagnostics: fresh,
        standing,
        ...(document.template === null ? {} : { preview: { text: document.repaired.text, template: document.template } }),
        repaired: { text: document.repaired.text, cut: document.repaired.cut },
        pending: document.repaired.pending.map((open) => ({ kind: open.kind, ...(open.name === null ? {} : { name: open.name }), start: open.start })),
        withheld: document.repaired.withheld,
      };

      last = step;

      if (elapsedMs > budgetMs) {
        ended = { code: "check-budget-exceeded", elapsedMs };

        return { ...step, ended };
      }

      return step;
    },
    async close() {
      if (closed) {
        throw new MeshUsageError("this session is closed");
      }

      closed = true;

      return check({ source: text + carry, path, model });
    },
  };
}
