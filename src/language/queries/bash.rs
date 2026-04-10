pub const QUERY: &str = r#"
; --- Functions ---

(function_definition
  name: (word) @name) @definition.function

; --- Top-level variable assignments ---

(program
  (variable_assignment
    name: (variable_name) @name) @definition.constant)
"#;
