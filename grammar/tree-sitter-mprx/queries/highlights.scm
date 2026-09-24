; Syntax highlighting for MPRX. The capture names are nvim-treesitter's.
;
; These queries colour text and decide nothing else: no MESH feature reads
; a capture. Diagnostics, hover, definition and completion come from the
; MESH compiler. A test in `bindings/rust/highlight_tests.rs` pins every
; capture on the files in `test/captures/`.

; Elements

(tag_name) @tag

(self_closing_element ["<" "/>"] @tag.delimiter)
(container_element ["<" ">" "</"] @tag.delimiter)

(attribute name: (identifier) @attribute)

; `on.` is part of an anonymous token that queries can't name, so only the
; event's name is captured.
(event_binding name: (identifier) @attribute)

(attribute "=" @punctuation.delimiter)
(event_binding "=" @punctuation.delimiter)

; Expressions

(expression_block ["{" "}"] @punctuation.special)

(string) @string

(number_literal) @number

(boolean_literal) @boolean

(null_literal) @constant.builtin

(event_value) @variable.builtin

(reference (identifier) @variable)

(member_access property: (identifier) @variable.member)
(member_access "." @punctuation.delimiter)

(command_invocation command: (identifier) @function.call)
(command_invocation ["(" ")"] @punctuation.bracket)
(command_invocation "," @punctuation.delimiter)

(object_member key: (identifier) @property)
(object_member ":" @punctuation.delimiter)
(object_expression ["{" "}"] @punctuation.bracket)
(object_expression "," @punctuation.delimiter)

(array_expression ["[" "]"] @punctuation.bracket)

; Grouping parentheses.
(expression ["(" ")"] @punctuation.bracket)
(array_expression "," @punctuation.delimiter)

(unary_expression operator: ["!" "-"] @operator)
(binary_expression
  operator: ["*" "/" "%" "+" "-" "<" "<=" ">" ">=" "==" "!=" "&&" "||"] @operator)
(conditional_expression ["?" ":"] @operator)
