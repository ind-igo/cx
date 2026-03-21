/// Simple glob matching: `*` matches any substring, `?` matches one character.
/// Case-sensitive. Uses an iterative algorithm (O(n*m), no exponential backtracking).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();

    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < txt.len() {
        if pi < pat.len() && (pat[pi] == '?' || pat[pi] == txt[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pat.len() && pat[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1; // try matching * with zero chars
        } else if star_pi != usize::MAX {
            // backtrack: let the last * consume one more char
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    // consume trailing *s
    while pi < pat.len() && pat[pi] == '*' {
        pi += 1;
    }

    pi == pat.len()
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

    #[test]
    fn test_adversarial_pattern() {
        // This would hang with recursive backtracking
        let text = "a".repeat(100);
        assert!(!glob_match("*a*a*a*a*a*b", &text));
    }
}
