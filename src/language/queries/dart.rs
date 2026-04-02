pub const QUERY: &str = r#"
; --- Type declarations ---
; sealed/base/interface/mixin class modifiers all produce class_definition

(class_definition
  name: (identifier) @name) @definition.class

(mixin_declaration
  (identifier) @name) @definition.class

(extension_declaration
  (identifier) @name) @definition.module

(extension_type_declaration
  (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.enum

(type_alias
  (type_identifier) @name) @definition.type

; --- Top-level functions ---

(program
  (function_signature
    (identifier) @name) @definition.function)

; --- Methods in class_body (shared by class, mixin class, extension type) ---

(class_body
  (method_signature
    (function_signature
      (identifier) @name)) @definition.method)

(class_body
  (method_signature
    (getter_signature
      (identifier) @name)) @definition.method)

(class_body
  (method_signature
    (setter_signature
      (identifier) @name)) @definition.method)

(class_body
  (method_signature
    (operator_signature
      "operator" @name)) @definition.method)

; --- Abstract methods (function_signature inside declaration in class_body) ---

(class_body
  (declaration
    (function_signature
      (identifier) @name)) @definition.method)

; --- Methods in extension_body ---

(extension_body
  (method_signature
    (function_signature
      (identifier) @name)) @definition.method)

(extension_body
  (method_signature
    (getter_signature
      (identifier) @name)) @definition.method)

(extension_body
  (method_signature
    (setter_signature
      (identifier) @name)) @definition.method)

(extension_body
  (method_signature
    (operator_signature
      "operator" @name)) @definition.method)

; --- Methods in enum_body ---

(enum_body
  (method_signature
    (function_signature
      (identifier) @name)) @definition.method)

(enum_body
  (method_signature
    (getter_signature
      (identifier) @name)) @definition.method)

(enum_body
  (method_signature
    (setter_signature
      (identifier) @name)) @definition.method)

(enum_body
  (method_signature
    (operator_signature
      "operator" @name)) @definition.method)

; --- Constructors (unnamed first, named second — later pattern wins in dedup) ---

(class_body
  (declaration
    (constructor_signature
      (identifier) @name)) @definition.method)

(class_body
  (declaration
    (constructor_signature
      (identifier) @_class
      "."
      (identifier) @name)) @definition.method)

; --- Factory constructors (unnamed first, named second) ---

(class_body
  (method_signature
    (factory_constructor_signature
      (identifier) @name)) @definition.method)

(class_body
  (method_signature
    (factory_constructor_signature
      (identifier) @_class
      "."
      (identifier) @name)) @definition.method)
"#;
