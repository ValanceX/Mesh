/**
 * The shapes the runtime returns, as its schemas describe them: the
 * render tree and the command intent (`schemas/render-v1.schema.json`),
 * the runtime diagnostics document
 * (`schemas/runtime-diagnostics-v1.schema.json`), and the host's
 * obligations. A later version may add properties; ignore any you don't
 * know. A PORT renderer needs only the tree's types.
 */

import type { Render } from "./engine.js";

/**
 * A value from the boundary data model (§9.8.1): null, a boolean, a
 * finite number (never `-0`), a string, a list, or a record. An absent
 * value is never one: an absent prop or field is simply missing.
 */
export type BoundaryValue =
  | null
  | boolean
  | number
  | string
  | readonly BoundaryValue[]
  | { readonly [field: string]: BoundaryValue };

/** What a renderer draws. Frozen: nothing in it can be changed. */
export interface RenderTree {
  readonly format: "mesh-render";
  readonly version: 1;
  readonly root: RenderNode;
}

/** A primitive occurrence. Composites never appear: they're expanded. */
export interface RenderNode {
  readonly type: "node";
  /** Its structural identity: unique in the tree, the same in every render of the program. Opaque. */
  readonly key: string;
  /** The primitive component's name. */
  readonly component: string;
  /** Each written prop's value. An absent prop has no member; a `null` prop is `null`. */
  readonly props: { readonly [prop: string]: BoundaryValue };
  /** Each event binding's handler identifier, by event name. Opaque: report it, never interpret it. */
  readonly events: { readonly [event: string]: string };
  readonly children: readonly (RenderNode | TextRun)[];
}

/** The text of a run of adjacent literal text and interpolations. Present even when empty. */
export interface TextRun {
  readonly type: "text";
  readonly key: string;
  readonly text: string;
}

/** Which command a handler invoked, and its evaluated arguments. Only for the host. */
export interface CommandIntent {
  readonly command: {
    /** The component whose template declares the command. */
    readonly component: string;
    readonly name: string;
  };
  /** One entry per parameter, in order. */
  readonly arguments: readonly IntentArgument[];
}

/** A present argument, or an absent one (where the parameter is optional), which isn't `null`. */
export type IntentArgument = { readonly value: BoundaryValue } | { readonly absent: true };

/**
 * The runtime diagnostics document, as
 * `schemas/runtime-diagnostics-v1.schema.json` describes it: what
 * render and dispatch report instead of a result, and what `mesh
 * check-program --format json` prints. A later version may add
 * properties; ignore any you don't know.
 */
export interface RuntimeDiagnosticsDocument {
  /** The version of the document's shape. */
  version: 1;
  /** Every diagnostic, in the order MESH reports them. */
  diagnostics: RuntimeDiagnostic[];
}

export interface RuntimeDiagnostic {
  /** Always `error`: the runtime has no warnings. */
  severity: "error";
  /** The stable code, such as `"assembly-cycle"`. Match on this, not on `message`. */
  code: string;
  /** The human-readable message. It may change between versions. */
  message: string;
  location: RuntimeLocation;
}

/** Where a runtime diagnostic is: one of six forms, by `kind`. */
export type RuntimeLocation =
  /** In the model: a span in the manifest's text. */
  | { kind: "model"; span: ModelSpan }
  /** The program as a whole. */
  | { kind: "program" }
  /** A template, by its 0-based position in the list given, with its component when it can be read. */
  | { kind: "template"; index: number; component?: string }
  /** A place in a template: its component, and a span in the source it was compiled from. */
  | { kind: "source"; component: string; span: SourceSpan }
  /** A value the host gave: a path from a scope name, or from `$event`. */
  | { kind: "input"; path: (string | number)[] }
  /** The handler identifier dispatch was given. */
  | { kind: "handler" };

export interface ModelSpan {
  start: ModelPosition;
  end: ModelPosition;
}

/** A position in the manifest, as the diagnostics document gives one. */
export interface ModelPosition {
  /** 0-based byte offset into the UTF-8 text. */
  byte: number;
  /** 1-based line. */
  line: number;
  /** 1-based column, in characters. */
  column: number;
  /** 0-based offset in UTF-16 code units. */
  utf16: number;
  /** 1-based column, in UTF-16 code units. */
  utf16Column: number;
}

export interface SourceSpan {
  start: SourceOffset;
  end: SourceOffset;
}

/** An offset into MPRX source, as a template records it. */
export interface SourceOffset {
  byte: number;
  utf16: number;
}

/**
 * A host's obligations (docs/manual/runtime.md, "The host"). Nothing
 * checks that a host implements this; it names what a host must do.
 */
export interface Host {
  /**
   * Keeps each render while its tree is drawn, since dispatch needs the
   * render the renderer drew, not a newer one.
   */
  keep(render: Render): void;
  /**
   * Relays an event the renderer reported (a handler identifier from the
   * tree it drew, and a payload) to dispatch, with that tree's render.
   */
  relay(handler: string, payload?: unknown): void;
}
