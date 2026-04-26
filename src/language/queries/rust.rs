pub const QUERY: &str = r"
; --- Type definitions ---

(struct_item
    name: (type_identifier) @name) @definition.class

(enum_item
    name: (type_identifier) @name) @definition.class

(union_item
    name: (type_identifier) @name) @definition.class

(type_item
    name: (type_identifier) @name) @definition.class

; --- Functions & methods ---

(declaration_list
    (function_item
        name: (identifier) @name) @definition.method)

(function_item
    name: (identifier) @name) @definition.function

; --- Traits ---

(trait_item
    name: (type_identifier) @name) @definition.interface

; --- Modules ---

(mod_item
    name: (identifier) @name) @definition.module

; --- Macros ---

(macro_definition
    name: (identifier) @name) @definition.macro

; --- Constants & statics ---

(const_item
    name: (identifier) @name) @definition.constant

(static_item
    name: (identifier) @name) @definition.constant
";
