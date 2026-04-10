pub const QUERY: &str = r#"
; --- Functions ---

(function_declaration
  name: (identifier) @name) @definition.function

(function_declaration
  name: (dot_index_expression
    field: (identifier) @name)) @definition.function

(function_declaration
  name: (method_index_expression
    method: (identifier) @name)) @definition.method

; --- Top-level local variables (module tables, constants) ---

(chunk
  (variable_declaration
    (assignment_statement
      (variable_list
        name: (identifier) @name))) @definition.constant)
"#;
