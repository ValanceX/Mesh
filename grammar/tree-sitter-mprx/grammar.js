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
      field('name', $.identifier),
      repeat($.attribute),
      '/>',
    ),

    container_element: $ => seq(
      '<',
      field('name', $.identifier),
      repeat($.attribute),
      '>',
      optional(field('text', $.text)),
      '</',
      $.identifier,
      '>',
    ),

    attribute: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $.string),
    ),

    string: $ => seq(
      '"',
      optional(field('value', $.string_content)),
      '"',
    ),

    string_content: $ => /[^"]*/,

    text: $ => /[^<]+/,

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_-]*/,
  },
});
