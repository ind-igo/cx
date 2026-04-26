pub const QUERY: &str = r"
; --- Contracts, interfaces, libraries ---

(contract_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(library_declaration
  name: (identifier) @name) @definition.module

; --- Functions & modifiers ---

(function_definition
  name: (identifier) @name) @definition.function

(modifier_definition
  name: (identifier) @name) @definition.function

; --- Types ---

(struct_declaration
  name: (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.enum

(event_definition
  name: (identifier) @name) @definition.event

(error_declaration
  name: (identifier) @name) @definition.type

(user_defined_type_definition
  name: (identifier) @name) @definition.type

; --- State variables & constants ---

(state_variable_declaration
  name: (identifier) @name) @definition.field

(constant_variable_declaration
  name: (identifier) @name) @definition.constant
";
