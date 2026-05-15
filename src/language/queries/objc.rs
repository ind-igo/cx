pub const QUERY: &str = r#"
; --- Objective-C classes & protocols ---

(class_interface
  "@interface"
  .
  (identifier) @name) @definition.class

(class_implementation
  "@implementation"
  .
  (identifier) @name) @definition.class

(protocol_declaration
  "@protocol"
  .
  (identifier) @name) @definition.interface

; --- Objective-C methods ---

(method_definition
  (identifier) @name) @definition.method

(method_declaration
  (identifier) @name) @definition.method

(method_definition
  (method_identifier
    (identifier)? @name)) @definition.method

(method_declaration
  (method_identifier
    (identifier)? @name)) @definition.method

; --- C functions ---

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

(declaration
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(declaration
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function
"#;
