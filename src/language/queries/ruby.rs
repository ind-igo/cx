pub const QUERY: &str = r"
; --- Classes & modules ---

(class
  name: (constant) @name) @definition.class

(module
  name: (constant) @name) @definition.module

; --- Methods ---

(method
  name: (_) @name) @definition.method

(singleton_method
  name: (_) @name) @definition.method

(alias
  name: (_) @name) @definition.method

; --- Constants ---

(assignment
  left: (constant) @name) @definition.constant

(assignment
  left: (scope_resolution
    name: (constant) @name)) @definition.constant
";
