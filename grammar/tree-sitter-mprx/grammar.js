/**
 * @file Mprx grammar for tree-sitter
 * @author Jack Miller
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

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
    ),

    literal: $ => choice(
      $.string,
      $.number_literal,
      $.boolean_literal,
      $.null_literal,
    ),

    number_literal: $ => /-?[0-9]+(\.[0-9]+)?/,

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
