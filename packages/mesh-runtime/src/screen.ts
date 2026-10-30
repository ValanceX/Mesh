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
  raise as raiseWith,
  render as renderWith,
  update as updateWith,
  MeshUsageError,
  Render,
} from "./engine.js";
import type { ProgramInput, UpdateResult } from "./engine.js";
import type { CommandIntent, RenderPatches, RenderTree, RuntimeDiagnosticsDocument, UnitStates } from "./types.js";

/**
 * A unit command's transition: the occurrence's state as the person saw it, and the command's arguments in order
 * (`undefined` for an absent one), to what it does. `changes` are the fields it changes; a field the caller owns
 * (`controllable`, and written) is not applied. `raise` is an event of the occurrence's component, and its payload,
 * to tell the caller (a controlled tab strip says what the person chose): the screen resolves it to the handler the
 * caller bound, and `dispatch` gives that intent.
 */
export type Transition = (state: Readonly<Record<string, unknown>>, ...args: unknown[]) => TransitionResult;

/** What a {@link Transition} gives. */
export interface TransitionResult {
  readonly changes?: Readonly<Record<string, unknown>>;
  readonly raise?: readonly [event: string, payload?: unknown];
}

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
 * or, for a unit command the screen ran, the occurrence and the patches to apply, with an `intent` for the host when
 * there is one (the handler the caller bound to the event the transition raised, or, when the transition changed a
 * field the caller owns and raised nothing, the unit's own intent, so the change is the caller's to make); or diagnostics.
 */
export type ScreenDispatchResult =
  | { readonly intent: CommandIntent; readonly occurrence?: never; readonly patches?: never; readonly diagnostics?: never }
  | { readonly intent?: CommandIntent; readonly occurrence: string; readonly patches: RenderPatches; readonly diagnostics?: never }
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
  /** The snapshot `#current` was made from, as the host gave it: a unit's change renders it again with new units. */
  #snapshot: Record<string, unknown> = {};
  /** The state of the occurrences a transition changed, by occurrence: the runtime's `units` input. */
  #occurrences: UnitStates = {};
  /** The calls in flight, in order. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Changes on `close()`, so a call made before it keeps nothing after it. */
  #epoch = 0;

  /** {@link openScreen} is the way to make one. Throws a `MeshUsageError` (`invalid-argument`) for arguments of the wrong type. */
  constructor(input: ScreenInput) {
    if (typeof input !== "object" || input === null || typeof input.program !== "object" || input.program === null || typeof input.model !== "string") {
      throw new MeshUsageError("openScreen() takes { program, model, transitions? }, with the manifest's text as model");
    }
    for (const commands of Object.values(input.transitions === undefined ? {} : input.transitions)) {
      if (typeof commands !== "object" || commands === null || Object.values(commands).some((each) => typeof each !== "function")) {
        throw new MeshUsageError("openScreen()'s transitions are { [component]: { [command]: (state, ...args) => ({ changes, raise }) } }");
      }
    }
    this.#input = input;
  }

  /**
   * Shows `snapshot`: the first time (or after `close()`), the tree to draw; after that, the patches that turn the
   * tree drawn into this snapshot's. On diagnostics nothing changes, and the tree drawn still dispatches. It rejects
   * as `render` and `update` do. Don't change a snapshot after showing it: a unit's change renders it again.
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
        this.#keep(made.render, snapshot, epoch);
        return { tree: made.render.tree };
      }
      const updated = await updateWith(current, snapshot, this.#occurrences);
      return updated.diagnostics ? { diagnostics: updated.diagnostics } : { patches: this.#replace(current, updated, snapshot, epoch) };
    });
  }

  /**
   * Dispatches an event the renderer reported on the tree on screen: its handler identifier and payload. Gives the
   * intent of an application's command, for the host to run. A command of a unit whose transition the screen was
   * given is run here instead: the occurrence's new state is held, the program rendered again with it, and the
   * result is the occurrence and the patches to apply, and the intent of what the transition raised, if the caller
   * bound a handler to it. Rejects with a `MeshUsageError` (`invalid-argument`) when nothing is shown or a transition
   * gives something other than `{ changes?, raise? }`, and otherwise as `dispatch` does.
   *
   * It dispatches with the render whose tree or patches the screen gave last, so the renderer applies each result
   * before it reports the next event: an event reported against an older tree is dispatched against the newer one.
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
      const result: unknown = transition(seen === undefined ? {} : seen.state, ...args);
      if (!isTransitionResult(result)) {
        throw new MeshUsageError(`the transition of ${intent.command.component}'s ${intent.command.name} must give { changes?, raise?: [event, payload] }`);
      }
      const owned = new Set(intent.controlled);
      const held: Record<string, unknown> = { ...this.#occurrences[occurrence] };
      let ownedChanged = false;
      for (const [field, value] of Object.entries(result.changes === undefined ? {} : result.changes)) {
        if (owned.has(field)) {
          ownedChanged = true;
        } else {
          held[field] = value;
        }
      }
      // What the transition raised resolves against the render the person saw, as the event did.
      let told: CommandIntent | undefined = ownedChanged ? intent : undefined;
      if (result.raise !== undefined) {
        const raised = await raiseWith(current, occurrence, result.raise[0], result.raise[1]);
        if (raised.diagnostics) {
          return { diagnostics: raised.diagnostics };
        }
        told = raised.intent;
      }
      const units = { ...this.#occurrences, [occurrence]: held };
      // The snapshot on screen, rendered again with the new units. Every render the screen keeps has its own snapshot,
      // so none is ever gone (`render-gone`), even after the module is replaced.
      const updated = await updateWith(current, this.#snapshot, units);
      if (updated.diagnostics) {
        return { diagnostics: updated.diagnostics };
      }
      if (epoch === this.#epoch) {
        this.#occurrences = units;
      }
      const patches = this.#replace(current, updated, this.#snapshot, epoch);
      return told === undefined ? { occurrence, patches } : { intent: told, occurrence, patches };
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
    this.#snapshot = {};
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
  #replace(
    previous: Render,
    updated: Exclude<UpdateResult, { readonly diagnostics: RuntimeDiagnosticsDocument }>,
    snapshot: Record<string, unknown>,
    epoch: number,
  ): RenderPatches {
    previous.release();
    this.#keep(updated.render, snapshot, epoch);
    return updated.patches;
  }

  /** Keeps `render` as the one on screen, unless the screen was closed since the call began; drops the state of occurrences it no longer has. */
  #keep(render: Render, snapshot: Record<string, unknown>, epoch: number): void {
    if (epoch !== this.#epoch) {
      render.release();
      return;
    }
    this.#current = render;
    this.#snapshot = snapshot;
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
  return new Screen(input);
}

/** A transition's result has the shape the screen reads (what is in it is the runtime's to judge). */
function isTransitionResult(result: unknown): result is TransitionResult {
  if (typeof result !== "object" || result === null) {
    return false;
  }
  const { changes, raise } = result as { readonly changes?: unknown; readonly raise?: unknown };
  const changesOk = changes === undefined || (typeof changes === "object" && changes !== null);
  return changesOk && (raise === undefined || (Array.isArray(raise) && typeof raise[0] === "string"));
}
