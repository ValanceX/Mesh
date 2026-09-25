/**
 * The runtime diagnostics document, as
 * `schemas/runtime-diagnostics-v1.schema.json` describes it: what
 * `mesh check-program --format json` prints, what {@link checkProgram}
 * returns, and what the runtime's render and dispatch report. A later
 * version may add properties; ignore any you don't know.
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
