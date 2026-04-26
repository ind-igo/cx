pub const QUERY: &str = r"
; --- Functions ---

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function

; --- Structs & unions ---

(struct_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

(union_specifier
  name: (type_identifier) @name
  body: (_)) @definition.class

; --- Enums ---

(enum_specifier
  name: (type_identifier) @name) @definition.enum

; --- Type definitions ---

(type_definition
  declarator: (type_identifier) @name) @definition.type

; --- Function prototypes ---

(declaration
  declarator: (function_declarator
    declarator: (identifier) @name)) @definition.function

(declaration
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @name))) @definition.function
";
