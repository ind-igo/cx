use serde::Serialize;
use toon_format::encode_default;

/// Encode any serializable value as TOON and print it.
/// Falls back to debug format on encoding error.
pub fn print_toon<T: Serialize>(value: &T) {
    match encode_default(value) {
        Ok(s) => print!("{}", s),
        Err(e) => eprintln!("cx: toon encoding error: {}", e),
    }
}

/// Encode any serializable value as pretty-printed JSON and print it.
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("cx: json encoding error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Sym {
        name: String,
        kind: String,
    }

    #[test]
    fn test_toon_encode_array() {
        let syms = vec![
            Sym { name: "foo".into(), kind: "fn".into() },
            Sym { name: "Bar".into(), kind: "struct".into() },
        ];
        let result = encode_default(&syms).unwrap();
        // Should produce tabular format
        assert!(result.contains("name"), "should have field names: {result}");
        assert!(result.contains("foo"), "should have value: {result}");
        assert!(result.contains("Bar"), "should have value: {result}");
    }

    #[test]
    fn test_toon_encode_object() {
        use std::collections::BTreeMap;
        let mut obj = BTreeMap::new();
        obj.insert("file", "src/main.rs");
        obj.insert("status", "unchanged");
        let result = encode_default(&obj).unwrap();
        assert!(result.contains("file"), "{result}");
        assert!(result.contains("src/main.rs"), "{result}");
    }
}
