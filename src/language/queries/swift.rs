pub const QUERY: &str = r#"
; --- Type declarations ---

(class_declaration
  "class"
  name: (type_identifier) @name) @definition.class

(class_declaration
  "struct"
  name: (type_identifier) @name) @definition.struct

(class_declaration
  "enum"
  name: (type_identifier) @name) @definition.enum

(class_declaration
  "actor"
  name: (type_identifier) @name) @definition.class

(class_declaration
  "extension"
  name: _ @name) @definition.module

(protocol_declaration
  name: (type_identifier) @name) @definition.interface

(typealias_declaration
  name: (type_identifier) @name) @definition.type

; --- Methods (functions inside type bodies) ---

(class_body
  (function_declaration
    name: (simple_identifier) @name) @definition.method)

(enum_class_body
  (function_declaration
    name: (simple_identifier) @name) @definition.method)

(protocol_body
  (protocol_function_declaration
    name: (simple_identifier) @name) @definition.method)

; --- Init / Deinit ---

(class_body
  (init_declaration
    name: _ @name) @definition.method)

(enum_class_body
  (init_declaration
    name: _ @name) @definition.method)

(class_body
  (deinit_declaration
    "deinit" @name) @definition.method)

; --- Subscripts ---

(class_body
  (subscript_declaration
    "subscript" @name) @definition.method)

(enum_class_body
  (subscript_declaration
    "subscript" @name) @definition.method)

; --- Properties ---

(class_body
  (property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

(enum_class_body
  (property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

(protocol_body
  (protocol_property_declaration
    name: (pattern
      bound_identifier: (simple_identifier) @name)) @definition.constant)

; --- Top-level functions ---

(function_declaration
  name: (simple_identifier) @name) @definition.function
"#;
