/**
 * @file Mprx grammar for tree-sitter
 * @author Jack Miller
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const PREC = {
  CONDITIONAL: 1,
  LOGICAL_OR: 2,
  LOGICAL_AND: 3,
  EQUALITY: 4,
  RELATIONAL: 5,
  ADDITIVE: 6,
  MULTIPLICATIVE: 7,
  UNARY: 8,
};

// `selfClosingElement` / `containerElement` are plain JS helper functions
// (not grammar rules) returning the grammar expression shared by the
// root-only `self_closing_element` / `container_element` rules and their
// `_nested_*` twins below. tree-sitter assigns a distinct grammar symbol
// per rule *key* in the `rules` object, not per function reference, so
// both the root rule and its nested twin still compile to separate
// symbols even though they call the same helper — this only removes the
// duplication in the JS source, it does not change the generated parser's
// behavior. Extracting the body here is what lets both call sites be kept
// in sync in one edit going forward (see the comment above
// `_nested_self_closing_element` for why the two call sites need to exist
// as separate symbols in the first place).
const selfClosingElement = $ => seq(
  '<',
  field('name', $.tag_name),
  repeat($.attribute),
  '/>',
);

const containerElement = $ => seq(
  '<',
  field('name', $.tag_name),
  repeat($.attribute),
  '>',
  repeat($.child),
  '</',
  $.tag_name,
  '>',
);

module.exports = grammar({
  name: 'mprx',

  extras: $ => [/\s/],

  rules: {
    source_file: $ => $.element,

    element: $ => choice(
      $.self_closing_element,
      $.container_element,
    ),

    self_closing_element: selfClosingElement,

    container_element: containerElement,

    child: $ => choice(
      $.text,
      $.expression_block,
      alias($._nested_element, $.element),
    ),

    // `_nested_element` (and the `_nested_self_closing_element` /
    // `_nested_container_element` rules below it) are structurally
    // identical to `element` / `self_closing_element` / `container_element`
    // above (both pairs share the `selfClosingElement`/`containerElement`
    // helpers defined at the top of this file), but are distinct grammar
    // symbols, all aliased back to the same node names so the CST shape —
    // and every consumer that matches on `"element"` /
    // `"self_closing_element"` / `"container_element"` node kinds — is
    // identical either way.
    //
    // The duplication exists because `element` is only ever reached from
    // `source_file` (FOLLOW = end of input) while this path is only ever
    // reached from `child` (FOLLOW = another child or a close tag).
    // tree-sitter's parser-state merging (it builds LR(1) item sets, then
    // merges states with compatible cores, dropping lookahead-only
    // distinctions) merges states whose item cores are identical — and
    // the interior states of `self_closing_element`/`container_element`
    // (reached while still consuming '<name ... />' or '<name ...>...') have
    // an identical core regardless of which outer rule will eventually
    // reduce them. Aliasing only the outer `element` layer (leaving
    // `self_closing_element`/`container_element` shared) still merges those
    // interior states, so the lexer offers `text` as a candidate token
    // immediately after completing the root element too — this is a
    // lexer/token-choice ambiguity from that state sharing, not a grammar
    // conflict `tree-sitter generate` would catch — and greedily consumes
    // trailing whitespace after the root element as a spurious `text`
    // node, producing an unrecoverable parse error. Duplicating
    // `self_closing_element`/`container_element` themselves gives the two
    // call sites distinct item cores from their first token, so their
    // states no longer merge. Sharing the helper functions above (rather
    // than the duplicated rule bodies previously here) means the two
    // copies can no longer drift out of sync — a future edit to one call
    // site (e.g. Pass 4b's event bindings) necessarily updates both.
    _nested_element: $ => choice(
      alias($._nested_self_closing_element, $.self_closing_element),
      alias($._nested_container_element, $.container_element),
    ),

    _nested_self_closing_element: selfClosingElement,

    _nested_container_element: containerElement,

    attribute: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', choice($.string, $.expression_block)),
    ),

    expression_block: $ => seq(
      '{',
      field('expression', $.expression),
      '}',
    ),

    expression: $ => choice(
      $.literal,
      $.reference,
      $.member_access,
      $.unary_expression,
      $.binary_expression,
      $.conditional_expression,
      $.array_expression,
      $.object_expression,
      $.command_invocation,
      $.event_value,
      seq('(', $.expression, ')'),
    ),

    unary_expression: $ => prec(PREC.UNARY, seq(
      field('operator', choice('!', '-')),
      field('operand', $.expression),
    )),

    binary_expression: $ => choice(
      prec.left(PREC.MULTIPLICATIVE, seq(
        field('left', $.expression),
        field('operator', choice('*', '/', '%')),
        field('right', $.expression),
      )),
      prec.left(PREC.ADDITIVE, seq(
        field('left', $.expression),
        field('operator', choice('+', '-')),
        field('right', $.expression),
      )),
      prec.left(PREC.RELATIONAL, seq(
        field('left', $.expression),
        field('operator', choice('<', '<=', '>', '>=')),
        field('right', $.expression),
      )),
      prec.left(PREC.EQUALITY, seq(
        field('left', $.expression),
        field('operator', choice('==', '!=')),
        field('right', $.expression),
      )),
      prec.left(PREC.LOGICAL_AND, seq(
        field('left', $.expression),
        field('operator', '&&'),
        field('right', $.expression),
      )),
      prec.left(PREC.LOGICAL_OR, seq(
        field('left', $.expression),
        field('operator', '||'),
        field('right', $.expression),
      )),
    ),

    conditional_expression: $ => prec.right(PREC.CONDITIONAL, seq(
      field('condition', $.expression),
      '?',
      field('consequent', $.expression),
      ':',
      field('alternate', $.expression),
    )),

    array_expression: $ => seq(
      '[',
      optional(seq(
        $.expression,
        repeat(seq(',', $.expression)),
        optional(','),
      )),
      ']',
    ),

    object_expression: $ => seq(
      '{',
      optional(seq(
        $.object_member,
        repeat(seq(',', $.object_member)),
        optional(','),
      )),
      '}',
    ),

    object_member: $ => seq(
      field('key', choice($.identifier, $.string)),
      ':',
      field('value', $.expression),
    ),

    command_invocation: $ => seq(
      field('command', $.identifier),
      '(',
      optional(seq(
        $.expression,
        repeat(seq(',', $.expression)),
      )),
      ')',
    ),

    event_value: $ => /\$[a-zA-Z_][a-zA-Z0-9_]*/,

    literal: $ => choice(
      $.string,
      $.number_literal,
      $.boolean_literal,
      $.null_literal,
    ),

    number_literal: $ => /[0-9]+(\.[0-9]+)?/,

    boolean_literal: $ => choice('true', 'false'),

    null_literal: $ => 'null',

    reference: $ => $.identifier,

    member_access: $ => seq(
      field('object', choice($.reference, $.member_access)),
      '.',
      field('property', $.identifier),
    ),

    string: $ => seq(
      '"',
      optional(field('value', $.string_content)),
      '"',
    ),

    string_content: $ => /([^"\\]|\\.)*/,

    text: $ => /[^<{]+/,

    tag_name: $ => /[a-zA-Z_][a-zA-Z0-9_-]*/,

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
  },
});
