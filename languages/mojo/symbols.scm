(function_declaration
  (function_signature name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])) @definition.function

(struct_declaration
  (struct_header name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])) @definition.struct

(trait_declaration
  (trait_header name: (identifier) @name)) @definition.trait

(module
  (comptime_statement left: [
    (comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
    (constrained_comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
  ]) @definition.constant)

(struct_declaration body: (block
  (comptime_statement left: [
    (comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
    (constrained_comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
  ]) @definition.constant))

(trait_declaration body: (block
  (comptime_statement left: [
    (comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
    (constrained_comptime_parameter name: [(identifier) @name (escaped_identifier (escaped_identifier_content) @name)])
  ]) @definition.constant))
