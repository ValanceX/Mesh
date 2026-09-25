/**
 * The diagnostics document, as `schemas/diagnostics-v1.schema.json`
 * describes it: what `mesh check --format json` prints, and what
 * {@link check} returns. A later version may add properties; ignore any
 * you don't know.
 */
export interface DiagnosticsDocument {
  /** The version of the document's shape. */
  version: 1;
  /** Every diagnostic, in the order MESH reports them. */
  diagnostics: Diagnostic[];
}

/** Only errors fail a check. */
export type Severity = "error" | "warning";

export interface Diagnostic {
  severity: Severity;
  /** The stable code, such as `"unknown-reference"`. Match on this, not on `message`. */
  code: string;
  /** The human-readable message. It may change between versions. */
  message: string;
  /** The path the span points into: the source's, or the manifest's for a `manifest-*` code. */
  path: string;
  span: Span;
  /** Replacements that would probably fix the problem, best first. Usually empty. */
  suggestions: Suggestion[];
}

export interface Suggestion {
  replacement: string;
  span: Span;
}

/** `end` is just past the span's last character. */
export interface Span {
  start: Position;
  end: Position;
}

export interface Position {
  /** A 0-based offset into the text's UTF-8 bytes; a byte-order mark counts. */
  byte: number;
  /** The 1-based line. */
  line: number;
  /** The 1-based column, in characters; a byte-order mark and a line's `\r` aren't columns. */
  column: number;
  /** The same place as `byte`, in UTF-16 code units: `source.slice(start.utf16, end.utf16)` is the span's text. */
  utf16: number;
  /** The 1-based column in UTF-16 code units, with `column`'s rules. */
  utf16Column: number;
}
