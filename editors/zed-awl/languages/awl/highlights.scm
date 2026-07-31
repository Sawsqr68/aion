((doc_comment) @comment.doc
 (#match? @comment.doc "^///"))

((doc_comment) @comment.doc
 (#match? @comment.doc "^//!"))

(comment) @comment
(string) @string
(raw_string) @string
(duration) @number
(float) @number
(integer) @number
(boolean) @constant.builtin
(builtin_type) @type.builtin
(type_identifier) @type

(workflow_declaration name: (identifier) @function)
(input_declaration name: (identifier) @variable.parameter)
(signal_declaration name: (identifier) @variable.parameter)
(outcome_declaration name: (identifier) @label)
(type_declaration name: (type_identifier) @type)
(const_declaration name: (identifier) @constant)
(worker_declaration name: (identifier) @type)
(action_declaration name: (identifier) @function)
(child_declaration name: (identifier) @function)
(subflow_declaration name: (identifier) @function)
(step_declaration name: (identifier) @function)

; Raw brace-balanced bodies (`json { … }` literals and inline `schema { … }`
; type doors) are captured verbatim by the lexer; strings inside them keep
; their own (string) @string capture from above.
(json_body) @string.special
(schema_body) @string.special

(keyword) @keyword

((keyword) @function
 (#any-of? @function "filter" "map" "sort" "count" "any" "all"))

(pipe_operator) @operator
(bind_operator) @operator
(comparison_operator) @operator
(optional_operator) @operator
(punctuation) @punctuation
