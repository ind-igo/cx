/// Simple glob matching: `*` matches any substring, `?` matches one character.
/// Case-sensitive.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_recurse(&pat, &txt)
}

fn glob_recurse(pat: &[char], txt: &[char]) -> bool {
    match (pat.first(), txt.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            // '*' matches zero or more characters
            // Try matching rest of pattern against current position (zero match)
            // or advance text by one and retry (consume one char)
            glob_recurse(&pat[1..], txt) || (!txt.is_empty() && glob_recurse(pat, &txt[1..]))
        }
        (Some('?'), Some(_)) => glob_recurse(&pat[1..], &txt[1..]),
        (Some(p), Some(t)) if *p == *t => glob_recurse(&pat[1..], &txt[1..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(glob_match("hello", "hello"));
    }

    #[test]
    fn test_no_match() {
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn test_star_prefix() {
        assert!(glob_match("*fee", "calculate_fee"));
    }

    #[test]
    fn test_star_suffix() {
        assert!(glob_match("calc*", "calculate_fee"));
    }

    #[test]
    fn test_star_middle() {
        assert!(glob_match("calc*fee", "calculate_fee"));
    }

    #[test]
    fn test_star_all() {
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn test_question_mark() {
        assert!(glob_match("fo?", "foo"));
        assert!(!glob_match("fo?", "fooo"));
    }

    #[test]
    fn test_empty_pattern_empty_text() {
        assert!(glob_match("", ""));
    }

    #[test]
    fn test_empty_pattern_nonempty_text() {
        assert!(!glob_match("", "foo"));
    }

    #[test]
    fn test_star_empty_text() {
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_case_sensitive() {
        assert!(!glob_match("Foo", "foo"));
    }
}
