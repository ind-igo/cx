pub const QUERY: &str = r#"
(Decl
  (FnProto
    (IDENTIFIER) @name)) @definition.function

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "struct")))))) @definition.class

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "enum")))))) @definition.enum

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ContainerDecl
          (ContainerDeclType
            "union")))))) @definition.class

(Decl
  (VarDecl
    (IDENTIFIER) @name
    (ErrorUnionExpr
      (SuffixExpr
        (ErrorSetDecl))))) @definition.enum
"#;
