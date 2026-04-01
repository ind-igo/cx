pub const QUERY: &str = r#"
(call
  target: (identifier) @_keyword
  (arguments (alias) @name)
  (#any-of? @_keyword "defmodule" "defprotocol" "defimpl")) @definition.module

(call
  target: (identifier) @_keyword
  (arguments
    [(identifier) @name
     (call target: (identifier) @name)
     (binary_operator left: (call target: (identifier) @name))])
  (#any-of? @_keyword "def" "defp" "defmacro" "defmacrop" "defguard" "defguardp" "defdelegate")) @definition.function

(unary_operator
  operand: (call
    target: (identifier) @_keyword
    (arguments
      (binary_operator
        left: (identifier) @name)))
  (#any-of? @_keyword "type" "typep" "opaque")) @definition.type

(unary_operator
  operand: (call
    target: (identifier) @_keyword
    (arguments
      (binary_operator
        left: (call target: (identifier) @name)))
    (#eq? @_keyword "callback"))) @definition.method
"#;
