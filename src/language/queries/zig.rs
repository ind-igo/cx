pub const QUERY: &str = r#"
; --- Functions ---

(Decl
  (FnProto
    (IDENTIFIER) @name)) @definition.function

; --- Structs ---

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "struct")))))) @definition.struct

; --- Enums ---

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "enum")))))) @definition.enum

; --- Unions ---

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "union")))))) @definition.class

; --- Error sets ---

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ErrorSetDecl))))) @definition.enum

; --- Constants (catch-all, overridden by struct/enum/union at same range) ---

(Decl
  (VarDecl
    (IDENTIFIER) @name)) @definition.constant

; --- Container fields (struct/union fields, enum variants) ---

(ContainerField
  (IDENTIFIER) @name) @definition.field

; --- Tests ---

(TestDecl
  (STRINGLITERALSINGLE) @name) @definition.function
"#;
