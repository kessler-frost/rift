//! Detects the `# ` natural-language prefix on terminal input.

/// If `input` begins with `# ` (hash + space), return the trimmed NL request.
/// Returns `None` for ordinary commands (including a bare `#` comment with no space).
pub fn nl_request(input: &str) -> Option<&str> {
    let rest = input.strip_prefix("# ")?;
    let rest = rest.trim();
    (!rest.is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hash_space_prefix() {
        assert_eq!(nl_request("# files changed today"), Some("files changed today"));
    }

    #[test]
    fn ignores_plain_commands() {
        assert_eq!(nl_request("ls -la"), None);
        assert_eq!(nl_request("#nospace"), None);
        assert_eq!(nl_request("echo # mid-line"), None);
    }

    #[test]
    fn ignores_empty_request() {
        assert_eq!(nl_request("#  "), None);
    }
}
