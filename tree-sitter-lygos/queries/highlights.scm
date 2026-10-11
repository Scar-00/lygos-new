(identifier) @variable
(self) @variable.builtin
(type_identifier) @type
(primitive_type) @type.builtin

(function_declaration name: (identifier) @function)
(parameter name: (identifier) @variable.parameter)
(field_declaration name: (identifier) @variable.member)
(field_expression field: (identifier) @variable.member)
(field_initializer name: (identifier) @variable.member)
(enum_variant name: (identifier) @constant)
(static_declaration name: (identifier) @constant)

(scoped_expression path: (identifier) @type name: (identifier) @constant)
(call_expression function: (identifier) @function.call)
(call_expression function: (field_expression field: (identifier) @function.method.call))
(call_expression function: (scoped_expression path: (identifier) @type name: (identifier) @function.call))
(macro_definition name: (identifier) @function.macro)
(macro_invocation name: (identifier) @function.macro)
(macro_parameter name: (identifier) @variable.parameter)
(attribute name: (identifier) @attribute)
(comptime_function (function_declaration name: (identifier) @function.macro))
(macro_variable) @variable.parameter

(integer_literal) @number
(float_literal) @number.float
(string_literal) @string
(char_literal) @character
(escape_sequence) @string.escape
[(line_comment) (block_comment)] @comment

["struct" "enum" "trait" "impl" "type" "macro" "static" "let"] @keyword
"fn" @keyword.function
["import" "as"] @keyword.import
["mut" "const"] @keyword.modifier
["if" "else"] @keyword.conditional
["for" "in"] @keyword.repeat
"return" @keyword.return
"break" @keyword

["+" "-" "*" "/" "%" "&" "!" "=" "==" "!=" "<" ">" "<=" ">=" "||" "&&" "->"] @operator
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[";" ":" "," "." "::"] @punctuation.delimiter
"$" @punctuation.special
"#" @punctuation.special
