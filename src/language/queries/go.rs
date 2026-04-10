pub const QUERY: &str = r#"
; --- Functions ---

(function_declaration
  name: (identifier) @name) @definition.function

(method_declaration
  name: (field_identifier) @name) @definition.method

; --- Structs (must come before generic type_spec) ---

(type_spec
  name: (type_identifier) @name
  type: (struct_type)) @definition.struct

; --- Interfaces ---

(type_spec
  name: (type_identifier) @name
  type: (interface_type)) @definition.interface

; --- Other named types (catch-all for type aliases, func types, etc.) ---

(type_spec
  name: (type_identifier) @name) @definition.type

(type_alias
  name: (type_identifier) @name) @definition.type

; --- Constants ---

(const_spec
  name: (identifier) @name) @definition.constant

; --- Interface method specs ---

(method_elem
  name: (field_identifier) @name) @definition.method

; --- Struct fields ---

(field_declaration
  name: (field_identifier) @name) @definition.field
"#;
