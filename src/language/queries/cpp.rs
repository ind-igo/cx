pub const QUERY: &str = r#"
; --- Functions (also covers in-class constructors) ---

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @name)) @definition.method

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @name))) @definition.method

; --- Destructors ---

(function_definition
  declarator: (function_declarator
    declarator: (destructor_name
      (identifier) @name))) @definition.function

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (destructor_name
        (identifier) @name)))) @definition.function

; --- Template functions ---

(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

(template_declaration
  (function_definition
    declarator: (function_declarator
      declarator: (qualified_identifier
        name: (identifier) @name)))) @definition.method

; --- Classes & structs ---

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(class_specifier
  name: (type_identifier) @name) @definition.class

(template_declaration
  (class_specifier
    name: (type_identifier) @name)) @definition.class

(template_declaration
  (struct_specifier
    name: (type_identifier) @name
    body: (_))) @definition.class

; --- Enums ---

(enum_specifier
  name: (type_identifier) @name) @definition.enum

; --- Type definitions & aliases ---

(type_definition
  declarator: (type_identifier) @name) @definition.type

(alias_declaration
  name: (type_identifier) @name) @definition.type

; --- Namespaces ---

(namespace_definition
  name: (namespace_identifier) @name) @definition.module

; --- Concepts (C++20) ---

(concept_definition
  name: (identifier) @name) @definition.type
"#;
