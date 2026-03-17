pub fn toon_table(name: &str, fields: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("{}[{}]{{{}}}:\n", name, rows.len(), fields.join(","));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|v| toon_escape(v)).collect();
        out.push_str(&format!("  {}\n", cells.join(",")));
    }
    out
}

pub fn toon_escape(value: &str) -> String {
    if value.contains(',') || value.contains('\n') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

pub fn toon_scalar(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| {
            if v.contains('\n') {
                format!(
                    "{}: |\n{}\n",
                    k,
                    v.lines()
                        .map(|l| format!("  {}", l))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                format!("{}: {}\n", k, v)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toon_table_basic() {
        let result = toon_table(
            "symbols",
            &["name", "kind"],
            &[
                vec!["foo".into(), "fn".into()],
                vec!["Bar".into(), "struct".into()],
            ],
        );
        assert_eq!(result, "symbols[2]{name,kind}:\n  foo,fn\n  Bar,struct\n");
    }

    #[test]
    fn test_toon_table_empty() {
        let result = toon_table("symbols", &["name", "kind"], &[]);
        assert_eq!(result, "symbols[0]{name,kind}:\n");
    }

    #[test]
    fn test_toon_escape_plain() {
        assert_eq!(toon_escape("hello"), "hello");
    }

    #[test]
    fn test_toon_escape_comma() {
        assert_eq!(toon_escape("a, b"), "\"a, b\"");
    }

    #[test]
    fn test_toon_escape_newline() {
        assert_eq!(toon_escape("a\nb"), "\"a\nb\"");
    }

    #[test]
    fn test_toon_escape_quotes() {
        assert_eq!(toon_escape("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn test_toon_scalar_simple() {
        let result = toon_scalar(&[("file", "src/main.rs"), ("status", "unchanged")]);
        assert_eq!(result, "file: src/main.rs\nstatus: unchanged\n");
    }

    #[test]
    fn test_toon_scalar_multiline() {
        let result = toon_scalar(&[("body", "line one\nline two")]);
        assert_eq!(result, "body: |\n  line one\n  line two\n");
    }

    #[test]
    fn test_toon_table_with_signature() {
        let result = toon_table(
            "symbols",
            &["name", "kind", "signature"],
            &[vec![
                "calculate_fee".into(),
                "fn".into(),
                "fn calculate_fee(amount: U256, tier: FeeTier) -> U256".into(),
            ]],
        );
        assert_eq!(
            result,
            "symbols[1]{name,kind,signature}:\n  calculate_fee,fn,\"fn calculate_fee(amount: U256, tier: FeeTier) -> U256\"\n"
        );
    }
}
