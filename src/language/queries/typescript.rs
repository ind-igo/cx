pub const QUERY: &str = r#"
; --- Type declarations ---

(class_declaration
  name: (type_identifier) @name) @definition.class

(abstract_class_declaration
  name: (type_identifier) @name) @definition.class

(interface_declaration
  name: (type_identifier) @name) @definition.interface

(type_alias_declaration
  name: (type_identifier) @name) @definition.type

(enum_declaration
  name: (identifier) @name) @definition.enum

; --- Modules / namespaces ---

(module
  name: (identifier) @name) @definition.module

(internal_module
  name: (identifier) @name) @definition.module

; --- Functions ---

(function_declaration
  name: (identifier) @name) @definition.function

(lexical_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function))) @definition.function

(variable_declaration
  (variable_declarator
    name: (identifier) @name
    value: (arrow_function))) @definition.function

; --- Methods ---

(method_definition
  name: (property_identifier) @name) @definition.method

(abstract_method_signature
  name: (property_identifier) @name) @definition.method

(method_signature
  name: (property_identifier) @name) @definition.method

; --- Class fields ---

(public_field_definition
  name: (property_identifier) @name) @definition.field
"#;
