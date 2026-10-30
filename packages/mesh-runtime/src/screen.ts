/**
 * The screen: the Tier 1 way to host a program. It does the host's duties (docs/manual/runtime.md, "The host by hand"), so
 * the code that uses it only shows snapshots and dispatches the events its renderer reports: it keeps the render
 * whose tree is on screen, releases the one before it once its patches are handed over, gives an update the right
 * previous render and version, holds the state of the unit occurrences, and runs a unit's command on that state.
 *
 * Host-side, like `changes.ts`: the engine doesn't import it, and it calls only the package's public operations.
 * It reads an intent (which command, its arguments, its occurrence), as any host does, and judges no value: the
 * runtime validates the state a transition gives, as it does any input.
 */

import {
  dispatch as dispatchWith,
  render as renderWith,
  update as updateWith,
  updateChanges as updateChangesWith,
  MeshUsageError,
  Render,
} from "./engine.js";
import type { ProgramInput, UpdateResult } from "./engine.js";
import type { CommandIntent, RenderPatches, RenderTree, RuntimeDiagnosticsDocument, UnitStates } from "./types.js";

/**
 * A unit command's transition: the occurrence's state as the person saw it, and the command's arguments in order
 * (`undefined` for an absent one), to the fields it changes. A field the caller owns (`controllable`, and written)
 * is not applied.
 */
export type Transition = (state: Readonly<Record<string, unknown>>, ...args: unknown[]) => Readonly<Record<string, unknown>>;

/** What {@link openScreen} takes. */
export interface ScreenInput {
  readonly program: ProgramInput;
  /** The manifest's text: the model the templates were compiled against. */
  readonly model: string;
  /**
   * The transitions of the units' commands, by component, then command. Optional: a unit command with none comes
   * back from `dispatch` as its intent, for the host; a program with no units never needs it.
   */
  readonly transitions?: Readonly<Record<string, Readonly<Record<string, Transition>>>>;
}

/** What `show` gives: the first tree to draw, then the patches to apply to the tree drawn, or diagnostics. */
export type ShowResult =
  | { readonly tree: RenderTree; readonly patches?: never; readonly diagnostics?: never }
  | { readonly tree?: never; readonly patches: RenderPatches; readonly diagnostics?: never }
  | { readonly tree?: never; readonly patches?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/**
 * What `dispatch` gives: the intent of an application's command (or of a unit's with no transition), for the host;
 * or, for a unit command the screen ran, the occurrence it changed and the patches to apply; or diagnostics.
 */
export type ScreenDispatchResult =
  | { readonly intent: CommandIntent; readonly occurrence?: never; readonly patches?: never; readonly diagnostics?: never }
  | { readonly intent?: never; readonly occurrence: string; readonly patches: RenderPatches; readonly diagnostics?: never }
  | { readonly intent?: never; readonly occurrence?: never; readonly patches?: never; readonly diagnostics: RuntimeDiagnosticsDocument };

/**
 * One program on screen. Made by {@link openScreen}. Its calls run one at a time, in the order made, so a `show`
 * that isn't awaited still patches from the one before it. A renderer applies what each call gives before the next
 * event it reports: the screen dispatches with the render whose patches it last gave.
 */
export class Screen {
  readonly #input: ScreenInput;
  /** The render whose tree is on screen, once one is. */
  #current: Render | undefined;
  /** The state of the occurrences a transition changed, by occurrence: the runtime's `units` input. */
  #occurrences: UnitStates = {};
  /** The calls in flight, in order. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Changes on `close()`, so a call made before it keeps nothing after it. */
  #epoch = 0;

  /** Not public: a screen comes from {@link openScreen}. */
  constructor(input: ScreenInput) {
    this.#input = input;
  }

  /**
   * Shows `snapshot`: the first time (or after `close()`), the tree to draw; after that, the patches that turn the
   * tree drawn into this snapshot's. On diagnostics nothing changes, and the tree drawn still dispatches. It rejects
   * as `render` and `update` do.
   */
  show(snapshot: Record<string, unknown>): Promise<ShowResult> {
    const epoch = this.#epoch;
    return this.#run(async (): Promise<ShowResult> => {
      const current = this.#current;
      if (current === undefined) {
        const made = await renderWith({ ...this.#program(), snapshot, units: this.#occurrences });
        if (made.diagnostics) {
          return { diagnostics: made.diagnostics };
        }
        this.#keep(made.render, epoch);
        return { tree: made.render.tree };
      }
      const updated = await updateWith(current, snapshot, this.#occurrences);
      return updated.diagnostics ? { diagnostics: updated.diagnostics } : { patches: this.#replace(current, updated, epoch) };
    });
  }

  /**
   * Dispatches an event the renderer reported on the tree on screen: its handler identifier and payload. Gives the
   * intent of an application's command, for the host to run. A command of a unit whose transition the screen was
   * given is run here instead: the occurrence's new state is held, the program rendered again with it, and the
   * result is the occurrence and the patches to apply. Rejects with a `MeshUsageError` (`invalid-argument`) when
   * nothing is shown, and otherwise as `dispatch` does.
   */
  dispatch(handler: string, payload?: unknown): Promise<ScreenDispatchResult> {
    const epoch = this.#epoch;
    return this.#run(async (): Promise<ScreenDispatchResult> => {
      const current = this.#current;
      if (current === undefined) {
        throw new MeshUsageError("dispatch() needs a tree on screen: call show() first");
      }
      const dispatched = await dispatchWith(current, handler, payload);
      if (dispatched.diagnostics) {
        return { diagnostics: dispatched.diagnostics };
      }
      const { intent } = dispatched;
      const occurrence = intent.instance;
      const transition = occurrence === undefined ? undefined : this.#transition(intent.command.component, intent.command.name);
      if (occurrence === undefined || transition === undefined) {
        return { intent };
      }
      // The state the person saw: the render's, which has the caller's value for a field the caller owns.
      const seen = current.instances.find((each) => each.instance === occurrence);
      const args = intent.arguments.map((each) => ("value" in each ? each.value : undefined));
      const changes = transition(seen === undefined ? {} : seen.state, ...args);
      const owned = new Set(intent.controlled);
      const held: Record<string, unknown> = { ...this.#occurrences[occurrence] };
      for (const [field, value] of Object.entries(changes)) {
        if (!owned.has(field)) {
          held[field] = value;
        }
      }
      const units = { ...this.#occurrences, [occurrence]: held };
      // The snapshot is the one on screen: no edits, only new units, so nothing of the snapshot crosses again.
      const updated = await updateChangesWith(current, { base: current.version, changes: [] }, { units });
      if (updated.diagnostics) {
        return { diagnostics: updated.diagnostics };
      }
      if (epoch === this.#epoch) {
        this.#occurrences = units;
      }
      return { occurrence, patches: this.#replace(current, updated, epoch) };
    });
  }

  /**
   * Ends what is shown: releases the render on screen and lets go of the occurrences' state. Safe to call twice. The
   * screen stays usable: the next `show` gives a tree to draw afresh. A call still in flight keeps nothing.
   */
  close(): void {
    this.#epoch++;
    this.#current?.release();
    this.#current = undefined;
    this.#occurrences = {};
  }

  #program(): { readonly program: ProgramInput; readonly model: string } {
    return { program: this.#input.program, model: this.#input.model };
  }

  #transition(component: string, command: string): Transition | undefined {
    const transitions = this.#input.transitions;
    const commands = transitions !== undefined && Object.hasOwn(transitions, component) ? transitions[component] : undefined;
    return commands !== undefined && Object.hasOwn(commands, command) ? commands[command] : undefined;
  }

  /** `updated` is now on screen: its patches are handed over, so the render before it is released. */
  #replace(previous: Render, updated: Exclude<UpdateResult, { readonly diagnostics: RuntimeDiagnosticsDocument }>, epoch: number): RenderPatches {
    previous.release();
    this.#keep(updated.render, epoch);
    return updated.patches;
  }

  /** Keeps `render` as the one on screen, unless the screen was closed since the call began; drops the state of occurrences it no longer has. */
  #keep(render: Render, epoch: number): void {
    if (epoch !== this.#epoch) {
      render.release();
      return;
    }
    this.#current = render;
    const listed = new Set(render.instances.map((each) => each.instance));
    this.#occurrences = Object.fromEntries(Object.entries(this.#occurrences).filter(([occurrence]) => listed.has(occurrence)));
  }

  #run<T>(call: () => Promise<T>): Promise<T> {
    const result = this.#queue.then(call);
    this.#queue = result.catch(() => undefined);
    return result;
  }
}

/** `using screen = openScreen(...)` closes it at the end of the block, as `close()` does. */
export interface Screen {
  [Symbol.dispose](): void;
}
// Where the platform has `Symbol.dispose` (Node 20+, current browsers); elsewhere `close()` is the only spelling.
if (typeof Symbol.dispose === "symbol") {
  Object.defineProperty(Screen.prototype, Symbol.dispose, {
    value(this: Screen): void {
      this.close();
    },
    writable: true,
    configurable: true,
  });
}

/**
 * Opens a screen for `input.program`, compiled against `input.model`: nothing is rendered until the first `show`.
 * Throws a `MeshUsageError` (`invalid-argument`) for arguments of the wrong type.
 */
export function openScreen(input: ScreenInput): Screen {
  if (typeof input !== "object" || input === null || typeof input.program !== "object" || input.program === null || typeof input.model !== "string") {
    throw new MeshUsageError("openScreen() takes { program, model, transitions? }, with the manifest's text as model");
  }
  for (const commands of Object.values(input.transitions === undefined ? {} : input.transitions)) {
    if (typeof commands !== "object" || commands === null || Object.values(commands).some((each) => typeof each !== "function")) {
      throw new MeshUsageError("openScreen()'s transitions are { [component]: { [command]: (state, ...args) => changes } }");
    }
  }
  return new Screen(input);
}
