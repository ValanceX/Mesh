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

module.exports = grammar({
  name: 'mprx',

  extras: $ => [/\s/],

  rules: {
    source_file: $ => $.element,

    element: $ => choice(
      $.self_closing_element,
      $.container_element,
    ),

    self_closing_element: $ => seq(
      '<',
      field('name', $.tag_name),
      repeat($.attribute),
      '/>',
    ),

    container_element: $ => seq(
      '<',
      field('name', $.tag_name),
      repeat($.attribute),
      '>',
      repeat($.child),
      '</',
      $.tag_name,
      '>',
    ),

    child: $ => choice(
      $.text,
      $.expression_block,
    ),

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
