use anyhow::{anyhow, Result};

/// Parse command arguments string into individual arguments
/// Handles quoted strings and escapes using POSIX shell-style parsing
pub fn parse_cmd_args(args_str: &str) -> Result<Vec<String>> {
    shell_words::split(args_str).map_err(|e| anyhow!("Failed to parse cmd_args: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cmd_args_simple() {
        let args = parse_cmd_args("-c /etc/nginx.conf").unwrap();
        assert_eq!(args, vec!["-c", "/etc/nginx.conf"]);
    }

    #[test]
    fn test_parse_cmd_args_double_quoted() {
        let args = parse_cmd_args(r#"-c "/etc/nginx.conf""#).unwrap();
        assert_eq!(args, vec!["-c", "/etc/nginx.conf"]);
    }

    #[test]
    fn test_parse_cmd_args_path_with_spaces() {
        let args = parse_cmd_args(r#"-c "/path with spaces/file""#).unwrap();
        assert_eq!(args, vec!["-c", "/path with spaces/file"]);
    }

    #[test]
    fn test_parse_cmd_args_single_quoted() {
        let args = parse_cmd_args("--arg 'single quoted'").unwrap();
        assert_eq!(args, vec!["--arg", "single quoted"]);
    }

    #[test]
    fn test_parse_cmd_args_mixed_quotes() {
        let args = parse_cmd_args(r#"-c "/path with spaces/file" -g 'daemon off;'"#).unwrap();
        assert_eq!(
            args,
            vec!["-c", "/path with spaces/file", "-g", "daemon off;"]
        );
    }

    #[test]
    fn test_parse_cmd_args_multiple_spaces() {
        let args = parse_cmd_args("  -a   -b  ").unwrap();
        assert_eq!(args, vec!["-a", "-b"]);
    }

    #[test]
    fn test_parse_cmd_args_empty() {
        let args = parse_cmd_args("").unwrap();
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_cmd_args_escaped_quotes() {
        let args = parse_cmd_args(r#"-c "file\"name""#).unwrap();
        assert_eq!(args, vec!["-c", "file\"name"]);
    }

    #[test]
    fn test_parse_cmd_args_escaped_spaces() {
        let args = parse_cmd_args(r"-c file\ name").unwrap();
        assert_eq!(args, vec!["-c", "file name"]);
    }

    #[test]
    fn test_parse_cmd_args_unclosed_quote() {
        let result = parse_cmd_args(r#"-c "/etc/nginx.conf"#);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse cmd_args"));
    }

    #[test]
    fn test_parse_cmd_args_complex_example() {
        let args = parse_cmd_args(r"-c /etc/nginx/nginx.conf -g 'daemon off;'").unwrap();
        assert_eq!(
            args,
            vec!["-c", "/etc/nginx/nginx.conf", "-g", "daemon off;"]
        );
    }
}
