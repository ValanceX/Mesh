/**
 * The types of a `template-v1` document: one component's MPRX, checked
 * clean and compiled (see the MESH repository's
 * `docs/manual/templates.md` and `schemas/template-v1.schema.json`).
 *
 * A template is data to store and hand to the MESH runtime, not to
 * interpret: nothing in this package reads one. The format may gain
 * properties without changing `version`; readers ignore ones they don't
 * know.
 */

/** A place in the source: UTF-8 bytes, and UTF-16 code units, from its start. */
export interface Offset {
  byte: number;
  utf16: number;
}

/** Where a construct was in the source. Nothing depends on it for meaning. */
export interface TemplateSpan {
  start: Offset;
  end: Offset;
}

export interface Template {
  format: "mesh-template";
  version: 1;
  /** The component whose template this is. */
  component: string;
  /** `sha256:` and 64 hexadecimal digits: the model it was checked against. */
  fingerprint: string;
  /** The compiler version that produced it: provenance only. */
  compiler: string;
  root: TemplateElement;
}

export interface TemplateElement {
  component: string;
  props: { prop: string; value: TemplateExpression; span: TemplateSpan }[];
  events: {
    event: string;
    command: string;
    arguments: TemplateExpression[];
    span: TemplateSpan;
  }[];
  children: TemplateChild[];
  span: TemplateSpan;
}

export type TemplateChild =
  | { kind: "text"; value: string; span: TemplateSpan }
  | { kind: "expression"; expression: TemplateExpression }
  | { kind: "element"; element: TemplateElement };

export type TemplateExpression =
  | { kind: "literal"; value: string | number | boolean | null; span: TemplateSpan }
  | { kind: "scope"; name: string; span: TemplateSpan }
  | { kind: "member"; object: TemplateExpression; field: string; span: TemplateSpan }
  | { kind: "unary"; operator: "not" | "negate"; operand: TemplateExpression; span: TemplateSpan }
  | {
      kind: "binary";
      operator:
        | "add" | "subtract" | "multiply" | "divide" | "remainder"
        | "equal" | "not-equal" | "less" | "less-equal" | "greater" | "greater-equal"
        | "and" | "or";
      left: TemplateExpression;
      right: TemplateExpression;
      span: TemplateSpan;
    }
  | {
      kind: "conditional";
      condition: TemplateExpression;
      consequent: TemplateExpression;
      alternate: TemplateExpression;
      span: TemplateSpan;
    }
  | { kind: "list"; elements: TemplateExpression[]; span: TemplateSpan }
  | {
      kind: "record";
      fields: { name: string; value: TemplateExpression; span: TemplateSpan }[];
      span: TemplateSpan;
    }
  | { kind: "event"; span: TemplateSpan };
