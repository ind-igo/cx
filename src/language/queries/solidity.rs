pub const QUERY: &str = r#"
(contract_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(library_declaration
  name: (identifier) @name) @definition.module

(function_definition
  name: (identifier) @name) @definition.function

(struct_declaration
  name: (identifier) @name) @definition.class

(enum_declaration
  name: (identifier) @name) @definition.enum

(event_definition
  name: (identifier) @name) @definition.event
"#;
