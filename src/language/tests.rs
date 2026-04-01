use super::*;
use crate::index::{Symbol, SymbolKind};
use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();

fn init_grammar_cache() {
    INIT.call_once(|| {
        let config = tree_sitter_language_pack::PackConfig {
            cache_dir: Some(crate::lang::grammar_cache_dir()),
            ..Default::default()
        };
        tree_sitter_language_pack::configure(&config)
            .expect("failed to configure grammar cache");
    });
}

fn extract(lang: &str, src: &str, file: &str) -> Vec<Symbol> {
    init_grammar_cache();
    parse_and_extract(lang, src.as_bytes(), &PathBuf::from(file)).unwrap()
}

// --- Rust ---

#[test]
fn rust_function() {
    let src = "pub fn calculate_fee(amount: u64) -> u64 {\n    amount * 3 / 1000\n}";
    let syms = extract("rust", src, "test.rs");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "calculate_fee");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
    assert!(!syms[0].signature.contains('{'));
    assert!(syms[0].signature.contains("pub fn"));
}

#[test]
fn rust_struct() {
    let src = "pub struct FeeConfig {\n    pub rate: u64,\n}";
    let syms = extract("rust", src, "test.rs");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "FeeConfig");
    assert_eq!(syms[0].kind, SymbolKind::Struct);
}

#[test]
fn rust_enum() {
    let src = "pub enum FeeTier {\n    Low,\n    High,\n}";
    let syms = extract("rust", src, "test.rs");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "FeeTier");
    assert_eq!(syms[0].kind, SymbolKind::Enum);
}

#[test]
fn rust_trait() {
    let src = "pub trait Configurable {\n    fn configure(&self);\n}";
    let syms = extract("rust", src, "test.rs");
    let trait_sym = syms.iter().find(|s| s.name == "Configurable").unwrap();
    assert_eq!(trait_sym.kind, SymbolKind::Trait);
}

#[test]
fn rust_multiple_symbols() {
    let src = "pub fn foo() {}\nfn bar() {}\npub struct Baz;";
    let syms = extract("rust", src, "test.rs");
    assert!(syms.len() >= 3);
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"bar"));
    assert!(names.contains(&"Baz"));
}

#[test]
fn rust_byte_range() {
    let src = "pub fn test_func() -> u32 { 42 }";
    let syms = extract("rust", src, "test.rs");
    assert_eq!(syms.len(), 1);
    let (start, end) = syms[0].byte_range;
    assert!(start < end);
    assert!(end <= src.len());
    assert!(src[start..end].contains("test_func"));
}

// --- TypeScript ---

#[test]
fn ts_function() {
    let src = "function greet(name: string): string { return name; }";
    let syms = extract("typescript", src, "test.ts");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "greet");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
}

#[test]
fn ts_class() {
    let src = "export class UserService {\n  getName() { return 'test'; }\n}";
    let syms = extract("typescript", src, "test.ts");
    let class = syms.iter().find(|s| s.name == "UserService").unwrap();
    assert_eq!(class.kind, SymbolKind::Class);
    let method = syms.iter().find(|s| s.name == "getName").unwrap();
    assert_eq!(method.kind, SymbolKind::Method);
}

#[test]
fn ts_interface() {
    let src = "export interface Config {\n  host: string;\n  port: number;\n}";
    let syms = extract("typescript", src, "test.ts");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Config");
    assert_eq!(syms[0].kind, SymbolKind::Interface);
}

#[test]
fn ts_arrow_function() {
    let src = "const add = (a: number, b: number) => a + b;";
    let syms = extract("typescript", src, "test.ts");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "add");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
    assert!(syms[0].signature.contains("const add"), "should include const: {}", syms[0].signature);
    assert!(!syms[0].signature.contains("a + b"), "should not include body: {}", syms[0].signature);
}

#[test]
fn ts_tsx() {
    let src = "export function App() { return <div />; }";
    let syms = extract("typescript", src, "test.tsx");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "App");
}

// --- Python ---

#[test]
fn py_function() {
    let src = "def greet(name: str) -> str:\n    return f'Hello, {name}'";
    let syms = extract("python", src, "test.py");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "greet");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
    assert!(syms[0].signature.contains("-> str"), "should preserve return type: {}", syms[0].signature);
    assert!(syms[0].signature.contains("name: str"), "should preserve param types: {}", syms[0].signature);
}

#[test]
fn py_class() {
    let src = "class UserService:\n    def get_name(self):\n        return 'test'";
    let syms = extract("python", src, "test.py");
    let class = syms.iter().find(|s| s.name == "UserService").unwrap();
    assert_eq!(class.kind, SymbolKind::Class);
}

#[test]
fn py_constant() {
    let src = "MAX_SIZE = 100";
    let syms = extract("python", src, "test.py");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "MAX_SIZE");
    assert_eq!(syms[0].kind, SymbolKind::Const);
}

#[test]
fn py_multiple_symbols() {
    let src = "def foo():\n    pass\n\ndef bar():\n    pass\n\nclass Baz:\n    pass";
    let syms = extract("python", src, "test.py");
    assert!(syms.len() >= 3);
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"bar"));
    assert!(names.contains(&"Baz"));
}

#[test]
fn py_type_annotation_preserved() {
    let src = "def foo(x: int, y: list[str]) -> bool:\n    return True";
    let syms = extract("python", src, "test.py");
    assert_eq!(syms.len(), 1);
    assert!(syms[0].signature.contains("int"), "sig: {}", syms[0].signature);
    assert!(syms[0].signature.contains("bool"), "sig: {}", syms[0].signature);
}

// --- Go ---

#[test]
fn go_function() {
    let src = "func Calculate(amount int) int {\n\treturn amount * 3\n}";
    let syms = extract("go", src, "test.go");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Calculate");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
    assert!(syms[0].signature.contains("func"), "sig: {}", syms[0].signature);
}

#[test]
fn go_method() {
    let src = "func (s *Server) Start() error {\n\treturn nil\n}";
    let syms = extract("go", src, "test.go");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Start");
    assert_eq!(syms[0].kind, SymbolKind::Method);
}

#[test]
fn go_type() {
    let src = "type Config struct {\n\tHost string\n}";
    let syms = extract("go", src, "test.go");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Config");
    assert_eq!(syms[0].kind, SymbolKind::Type);
}

// --- C ---

#[test]
fn c_function() {
    let src = "int calculate(int amount) {\n    return amount * 3;\n}";
    let syms = extract("c", src, "test.c");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "calculate");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
}

#[test]
fn c_pointer_returning_function() {
    let src = "char *strdup(const char *s) {\n    return NULL;\n}";
    let syms = extract("c", src, "test.c");
    let f = syms.iter().find(|s| s.name == "strdup");
    assert!(f.is_some(), "should find pointer-returning fn: {:?}", syms);
    assert_eq!(f.unwrap().kind, SymbolKind::Fn);
}

#[test]
fn c_struct() {
    let src = "struct Config {\n    int rate;\n};";
    let syms = extract("c", src, "test.c");
    let s = syms.iter().find(|s| s.name == "Config");
    assert!(s.is_some(), "should find struct: {:?}", syms);
    assert_eq!(s.unwrap().kind, SymbolKind::Struct);
}

// --- C++ ---

#[test]
fn cpp_class() {
    let src = "class Server {\npublic:\n    void start();\n};";
    let syms = extract("cpp", src, "test.cpp");
    let class = syms.iter().find(|s| s.name == "Server");
    assert!(class.is_some(), "should find class: {:?}", syms);
    assert_eq!(class.unwrap().kind, SymbolKind::Class);
}

// --- Java ---

#[test]
fn java_class_and_method() {
    let src = "public class UserService {\n    public String getName() {\n        return \"test\";\n    }\n}";
    let syms = extract("java", src, "Test.java");
    let class = syms.iter().find(|s| s.name == "UserService");
    assert!(class.is_some(), "should find class: {:?}", syms);
    assert_eq!(class.unwrap().kind, SymbolKind::Class);
}

// --- Ruby ---

#[test]
fn ruby_class_and_method() {
    let src = "class UserService\n  def get_name\n    'test'\n  end\nend";
    let syms = extract("ruby", src, "test.rb");
    let class = syms.iter().find(|s| s.name == "UserService");
    assert!(class.is_some(), "should find class: {:?}", syms);
    assert_eq!(class.unwrap().kind, SymbolKind::Class);
    let method = syms.iter().find(|s| s.name == "get_name");
    assert!(method.is_some(), "should find method: {:?}", syms);
}

// --- Lua ---

#[test]
fn lua_function() {
    let src = "function greet(name)\n    return 'Hello, ' .. name\nend";
    let syms = extract("lua", src, "test.lua");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "greet");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
}

// --- Zig ---

#[test]
fn zig_function() {
    let src = "pub fn calculate(amount: u64) u64 {\n    return amount * 3;\n}";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "calculate");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
}

#[test]
fn zig_struct() {
    let src = "const Point = struct {\n    x: f32,\n    y: f32,\n};";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1, "should find struct: {:?}", syms);
    assert_eq!(syms[0].name, "Point");
    assert_eq!(syms[0].kind, SymbolKind::Struct);
}

#[test]
fn zig_enum() {
    let src = "const Color = enum {\n    red,\n    green,\n    blue,\n};";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1, "should find enum: {:?}", syms);
    assert_eq!(syms[0].name, "Color");
    assert_eq!(syms[0].kind, SymbolKind::Enum);
}

#[test]
fn zig_union() {
    let src = "const Msg = union {\n    int: i32,\n    float: f64,\n};";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1, "should find union: {:?}", syms);
    assert_eq!(syms[0].name, "Msg");
    assert_eq!(syms[0].kind, SymbolKind::Struct);
}

#[test]
fn zig_pub_struct() {
    let src = "pub const Point = struct {\n    x: f32,\n    y: f32,\n};";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1, "should find pub struct: {:?}", syms);
    assert_eq!(syms[0].name, "Point");
    assert_eq!(syms[0].kind, SymbolKind::Struct);
}

#[test]
fn zig_error_set() {
    let src = "const MyError = error {\n    OutOfMemory,\n    InvalidInput,\n};";
    let syms = extract("zig", src, "test.zig");
    assert_eq!(syms.len(), 1, "should find error set: {:?}", syms);
    assert_eq!(syms[0].name, "MyError");
    assert_eq!(syms[0].kind, SymbolKind::Enum);
}

// --- Bash ---

#[test]
fn bash_function() {
    let src = "function greet() {\n    echo \"Hello\"\n}";
    let syms = extract("bash", src, "test.sh");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "greet");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
}

// --- Solidity ---

#[test]
fn solidity_contract_and_function() {
    let src = "contract Token {\n    function transfer(address to, uint amount) public {\n    }\n}";
    let syms = extract("solidity", src, "test.sol");
    let contract = syms.iter().find(|s| s.name == "Token");
    assert!(contract.is_some(), "should find contract: {:?}", syms);
    let func = syms.iter().find(|s| s.name == "transfer");
    assert!(func.is_some(), "should find function: {:?}", syms);
}

#[test]
fn solidity_event() {
    let src = "contract Token {\n    event Transfer(address indexed from, address indexed to, uint256 value);\n}";
    let syms = extract("solidity", src, "test.sol");
    let event = syms.iter().find(|s| s.name == "Transfer");
    assert!(event.is_some(), "should find event: {:?}", syms);
    assert_eq!(event.unwrap().kind, SymbolKind::Event);
}

// --- Elixir ---

#[test]
fn elixir_module_and_function() {
    let src = "defmodule MyApp.Users do\n  def get_user(id) do\n    id\n  end\nend";
    let syms = extract("elixir", src, "test.ex");
    let module = syms.iter().find(|s| s.name == "MyApp.Users");
    assert!(module.is_some(), "should find module: {:?}", syms);
    assert_eq!(module.unwrap().kind, SymbolKind::Module);
    let func = syms.iter().find(|s| s.name == "get_user");
    assert!(func.is_some(), "should find function: {:?}", syms);
    assert_eq!(func.unwrap().kind, SymbolKind::Fn);
}

#[test]
fn elixir_type_definitions() {
    let src = "defmodule MyApp do\n  @type status :: :active | :inactive\n  @typep internal :: map()\n  @opaque token :: binary()\nend";
    let syms = extract("elixir", src, "test.ex");
    let status = syms.iter().find(|s| s.name == "status");
    assert!(status.is_some(), "should find @type: {:?}", syms);
    assert_eq!(status.unwrap().kind, SymbolKind::Type);
    let internal = syms.iter().find(|s| s.name == "internal");
    assert!(internal.is_some(), "should find @typep: {:?}", syms);
    assert_eq!(internal.unwrap().kind, SymbolKind::Type);
    let token = syms.iter().find(|s| s.name == "token");
    assert!(token.is_some(), "should find @opaque: {:?}", syms);
    assert_eq!(token.unwrap().kind, SymbolKind::Type);
}

#[test]
fn elixir_callback() {
    let src = "defmodule MyBehaviour do\n  @callback validate(term()) :: :ok | {:error, term()}\n  @callback format(term()) :: String.t()\nend";
    let syms = extract("elixir", src, "test.ex");
    let validate = syms.iter().find(|s| s.name == "validate");
    assert!(validate.is_some(), "should find @callback validate: {:?}", syms);
    assert_eq!(validate.unwrap().kind, SymbolKind::Method);
    let format = syms.iter().find(|s| s.name == "format");
    assert!(format.is_some(), "should find @callback format: {:?}", syms);
    assert_eq!(format.unwrap().kind, SymbolKind::Method);
}

#[test]
fn elixir_defimpl() {
    let src = "defimpl String.Chars, for: MyApp.User do\n  def to_string(user), do: user.name\nend";
    let syms = extract("elixir", src, "test.ex");
    let impl_sym = syms.iter().find(|s| s.name == "String.Chars");
    assert!(impl_sym.is_some(), "should find defimpl: {:?}", syms);
    assert_eq!(impl_sym.unwrap().kind, SymbolKind::Module);
    let func = syms.iter().find(|s| s.name == "to_string");
    assert!(func.is_some(), "should find function in impl: {:?}", syms);
}

#[test]
fn elixir_protocol() {
    let src = "defprotocol Renderable do\n  @spec render(t()) :: String.t()\n  def render(data)\nend";
    let syms = extract("elixir", src, "test.ex");
    let proto = syms.iter().find(|s| s.name == "Renderable");
    assert!(proto.is_some(), "should find defprotocol: {:?}", syms);
    assert_eq!(proto.unwrap().kind, SymbolKind::Module);
}

// --- Swift ---

#[test]
fn swift_function() {
    let src = "func greet(name: String) -> String {\n    return \"Hello, \\(name)\"\n}";
    let syms = extract("swift", src, "test.swift");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "greet");
    assert_eq!(syms[0].kind, SymbolKind::Fn);
    assert!(syms[0].signature.contains("func greet"));
}

#[test]
fn swift_class_and_method() {
    let src = "class Animal {\n    func speak() -> String {\n        return \"...\"\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let cls = syms.iter().find(|s| s.name == "Animal");
    assert!(cls.is_some(), "should find class: {:?}", syms);
    assert_eq!(cls.unwrap().kind, SymbolKind::Class);
    let method = syms.iter().find(|s| s.name == "speak");
    assert!(method.is_some(), "should find method: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_struct() {
    let src = "struct Point {\n    var x: Double\n    var y: Double\n}";
    let syms = extract("swift", src, "test.swift");
    let s = syms.iter().find(|s| s.name == "Point");
    assert!(s.is_some(), "should find struct: {:?}", syms);
    assert_eq!(s.unwrap().kind, SymbolKind::Struct);
    let x = syms.iter().find(|s| s.name == "x");
    assert!(x.is_some(), "should find property x: {:?}", syms);
    assert_eq!(x.unwrap().kind, SymbolKind::Const);
    let y = syms.iter().find(|s| s.name == "y");
    assert!(y.is_some(), "should find property y: {:?}", syms);
    assert_eq!(y.unwrap().kind, SymbolKind::Const);
}

#[test]
fn swift_enum() {
    let src = "enum Direction {\n    case north, south, east, west\n}";
    let syms = extract("swift", src, "test.swift");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Direction");
    assert_eq!(syms[0].kind, SymbolKind::Enum);
}

#[test]
fn swift_enum_methods() {
    let src = "enum Direction {\n    case north, south\n    func opposite() -> Direction {\n        switch self {\n        case .north: return .south\n        default: return .north\n        }\n    }\n    init?(rawValue: String) {\n        switch rawValue {\n        case \"n\": self = .north\n        default: return nil\n        }\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let e = syms.iter().find(|s| s.name == "Direction");
    assert!(e.is_some(), "should find enum: {:?}", syms);
    assert_eq!(e.unwrap().kind, SymbolKind::Enum);
    let method = syms.iter().find(|s| s.name == "opposite");
    assert!(method.is_some(), "should find method in enum: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
    let init_sym = syms.iter().find(|s| s.name == "init");
    assert!(init_sym.is_some(), "should find init in enum: {:?}", syms);
    assert_eq!(init_sym.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_protocol() {
    let src = "protocol Drawable {\n    func draw()\n}";
    let syms = extract("swift", src, "test.swift");
    let proto = syms.iter().find(|s| s.name == "Drawable");
    assert!(proto.is_some(), "should find protocol: {:?}", syms);
    assert_eq!(proto.unwrap().kind, SymbolKind::Interface);
    let draw = syms.iter().find(|s| s.name == "draw");
    assert!(draw.is_some(), "should find protocol method: {:?}", syms);
    assert_eq!(draw.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_typealias() {
    let src = "typealias Callback = (Int) -> Void";
    let syms = extract("swift", src, "test.swift");
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].name, "Callback");
    assert_eq!(syms[0].kind, SymbolKind::Type);
}

#[test]
fn swift_init() {
    let src = "class Foo {\n    init(x: Int) {\n        self.x = x\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let init_sym = syms.iter().find(|s| s.name == "init");
    assert!(init_sym.is_some(), "should find init: {:?}", syms);
    assert_eq!(init_sym.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_deinit() {
    let src = "class Foo {\n    deinit {\n        print(\"bye\")\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let deinit_sym = syms.iter().find(|s| s.name == "deinit");
    assert!(deinit_sym.is_some(), "should find deinit: {:?}", syms);
    assert_eq!(deinit_sym.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_actor() {
    let src = "actor BankAccount {\n    var balance: Double\n    func deposit(_ amount: Double) {\n        balance += amount\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let actor = syms.iter().find(|s| s.name == "BankAccount");
    assert!(actor.is_some(), "should find actor: {:?}", syms);
    assert_eq!(actor.unwrap().kind, SymbolKind::Class);
    let method = syms.iter().find(|s| s.name == "deposit");
    assert!(method.is_some(), "should find actor method: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
    let prop = syms.iter().find(|s| s.name == "balance");
    assert!(prop.is_some(), "should find actor property: {:?}", syms);
    assert_eq!(prop.unwrap().kind, SymbolKind::Const);
}

#[test]
fn swift_extension() {
    let src = "extension String {\n    func reversed() -> String {\n        return String(self.reversed())\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let ext = syms.iter().find(|s| s.name == "String");
    assert!(ext.is_some(), "should find extension: {:?}", syms);
    assert_eq!(ext.unwrap().kind, SymbolKind::Module);
    let method = syms.iter().find(|s| s.name == "reversed");
    assert!(method.is_some(), "should find extension method: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_extension_constrained() {
    let src = "extension Array where Element: Comparable {\n    func sorted() -> [Element] {\n        return []\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let ext = syms.iter().find(|s| s.kind == SymbolKind::Module);
    assert!(ext.is_some(), "should find constrained extension: {:?}", syms);
    let method = syms.iter().find(|s| s.name == "sorted");
    assert!(method.is_some(), "should find method in constrained extension: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_subscript() {
    let src = "struct Matrix {\n    subscript(row: Int, col: Int) -> Double {\n        return 0.0\n    }\n}";
    let syms = extract("swift", src, "test.swift");
    let sub = syms.iter().find(|s| s.name == "subscript");
    assert!(sub.is_some(), "should find subscript: {:?}", syms);
    assert_eq!(sub.unwrap().kind, SymbolKind::Method);
}

#[test]
fn swift_protocol_property() {
    let src = "protocol Named {\n    var name: String { get }\n    func greet() -> String\n}";
    let syms = extract("swift", src, "test.swift");
    let proto = syms.iter().find(|s| s.name == "Named");
    assert!(proto.is_some(), "should find protocol: {:?}", syms);
    assert_eq!(proto.unwrap().kind, SymbolKind::Interface);
    let prop = syms.iter().find(|s| s.name == "name");
    assert!(prop.is_some(), "should find protocol property: {:?}", syms);
    assert_eq!(prop.unwrap().kind, SymbolKind::Const);
    let method = syms.iter().find(|s| s.name == "greet");
    assert!(method.is_some(), "should find protocol method: {:?}", syms);
    assert_eq!(method.unwrap().kind, SymbolKind::Method);
}

// --- find_references tests ---

#[test]
fn refs_rust_finds_all_usages() {
    init_grammar_cache();
    let src = "struct Foo { x: i32 }\nfn bar(f: Foo) -> Foo { f }";
    let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "Foo").unwrap();
    assert_eq!(refs.len(), 3, "should find struct def + 2 usages: {:?}", refs.iter().map(|r| r.line).collect::<Vec<_>>());
}

#[test]
fn refs_rust_no_match() {
    init_grammar_cache();
    let src = "fn main() {}";
    let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "nonexistent").unwrap();
    assert!(refs.is_empty());
}

#[test]
fn refs_line_column_correct() {
    init_grammar_cache();
    let src = "let x = 1;\nlet y = x + x;";
    let refs = find_references("rust", src.as_bytes(), &PathBuf::from("test.rs"), "x").unwrap();
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[0].line, 1);
    assert_eq!(refs[1].line, 2);
    assert_eq!(refs[2].line, 2);
}

#[test]
fn refs_typescript_identifier() {
    init_grammar_cache();
    let src = "const foo = 1;\nconsole.log(foo);";
    let refs = find_references("typescript", src.as_bytes(), &PathBuf::from("test.ts"), "foo").unwrap();
    assert_eq!(refs.len(), 2);
}

#[test]
fn refs_python_identifier() {
    init_grammar_cache();
    let src = "def greet(name):\n    return name";
    let refs = find_references("python", src.as_bytes(), &PathBuf::from("test.py"), "name").unwrap();
    assert_eq!(refs.len(), 2);
}

// --- is_test detection tests ---

#[test]
fn rust_test_attribute_detected() {
    let syms = extract("rust", "#[test]\nfn test_foo() { assert!(true); }", "test.rs");
    let sym = syms.iter().find(|s| s.name == "test_foo").unwrap();
    assert!(sym.is_test, "function with #[test] should be marked as test");
}

#[test]
fn rust_cfg_test_mod_detected() {
    let src = r#"
fn main() {}

#[cfg(test)]
mod tests {
    fn helper() {}
}
"#;
    let syms = extract("rust", src, "lib.rs");
    let main_sym = syms.iter().find(|s| s.name == "main").unwrap();
    assert!(!main_sym.is_test, "main should not be marked as test");
    let tests_mod = syms.iter().find(|s| s.name == "tests").unwrap();
    assert!(tests_mod.is_test, "#[cfg(test)] mod should be marked as test");
    // helper inside #[cfg(test)] mod should also be marked as test
    if let Some(helper) = syms.iter().find(|s| s.name == "helper") {
        assert!(helper.is_test, "function inside #[cfg(test)] mod should be marked as test");
    }
}

#[test]
fn rust_normal_fn_not_test() {
    let syms = extract("rust", "pub fn add(a: i32, b: i32) -> i32 { a + b }", "lib.rs");
    let sym = syms.iter().find(|s| s.name == "add").unwrap();
    assert!(!sym.is_test, "normal function should not be marked as test");
}

#[test]
fn rust_test_and_normal_mixed() {
    let src = r#"
pub fn real_fn() {}

#[test]
fn test_real_fn() {}

pub struct MyStruct;
"#;
    let syms = extract("rust", src, "lib.rs");
    let real_fn = syms.iter().find(|s| s.name == "real_fn").unwrap();
    assert!(!real_fn.is_test);
    let test_fn = syms.iter().find(|s| s.name == "test_real_fn").unwrap();
    assert!(test_fn.is_test);
    let my_struct = syms.iter().find(|s| s.name == "MyStruct").unwrap();
    assert!(!my_struct.is_test);
}
