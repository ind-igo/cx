pub const QUERY: &str = r#"
; --- Module-level constants ---

(module (assignment left: (identifier) @name) @definition.constant)

; --- Classes (broad pattern covers nested classes too) ---

(class_definition
  name: (identifier) @name) @definition.class

; --- Top-level functions ---

(module
  (function_definition
    name: (identifier) @name) @definition.function)

(module
  (decorated_definition
    definition: (function_definition
      name: (identifier) @name) @definition.function))

; --- Methods inside classes ---

(class_definition
  body: (block
    (function_definition
      name: (identifier) @name) @definition.method))

(class_definition
  body: (block
    (decorated_definition
      definition: (function_definition
        name: (identifier) @name) @definition.method)))
"#;
