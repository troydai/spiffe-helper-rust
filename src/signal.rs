use anyhow::{anyhow, Result};
use nix::sys::signal::Signal;

/// Parse a signal name string to a Signal enum
/// Accepts both "SIGHUP" and "HUP" formats (case-insensitive)
pub fn parse_signal_name(name: &str) -> Result<Signal> {
    let normalized = name.trim().to_uppercase();
    let signal_name = normalized.strip_prefix("SIG").unwrap_or(&normalized);

    match signal_name {
        "HUP" => Ok(Signal::SIGHUP),
        "INT" => Ok(Signal::SIGINT),
        "QUIT" => Ok(Signal::SIGQUIT),
        "TERM" => Ok(Signal::SIGTERM),
        "USR1" => Ok(Signal::SIGUSR1),
        "USR2" => Ok(Signal::SIGUSR2),
        "WINCH" => Ok(Signal::SIGWINCH),
        _ => Err(anyhow!("Unknown signal name: {}", name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_signal_name_sighup() {
        let signal = parse_signal_name("SIGHUP").unwrap();
        assert_eq!(signal, Signal::SIGHUP);
    }

    #[test]
    fn test_parse_signal_name_hup() {
        let signal = parse_signal_name("HUP").unwrap();
        assert_eq!(signal, Signal::SIGHUP);
    }

    #[test]
    fn test_parse_signal_name_lowercase() {
        let signal = parse_signal_name("sighup").unwrap();
        assert_eq!(signal, Signal::SIGHUP);
    }

    #[test]
    fn test_parse_signal_name_mixed_case() {
        let signal = parse_signal_name("SigHup").unwrap();
        assert_eq!(signal, Signal::SIGHUP);
    }

    #[test]
    fn test_parse_signal_name_with_whitespace() {
        let signal = parse_signal_name("  SIGHUP  ").unwrap();
        assert_eq!(signal, Signal::SIGHUP);
    }

    #[test]
    fn test_parse_signal_name_usr1() {
        let signal = parse_signal_name("SIGUSR1").unwrap();
        assert_eq!(signal, Signal::SIGUSR1);
    }

    #[test]
    fn test_parse_signal_name_usr2() {
        let signal = parse_signal_name("SIGUSR2").unwrap();
        assert_eq!(signal, Signal::SIGUSR2);
    }

    #[test]
    fn test_parse_signal_name_term() {
        let signal = parse_signal_name("SIGTERM").unwrap();
        assert_eq!(signal, Signal::SIGTERM);
    }

    #[test]
    fn test_parse_signal_name_int() {
        let signal = parse_signal_name("SIGINT").unwrap();
        assert_eq!(signal, Signal::SIGINT);
    }

    #[test]
    fn test_parse_signal_name_quit() {
        let signal = parse_signal_name("SIGQUIT").unwrap();
        assert_eq!(signal, Signal::SIGQUIT);
    }

    #[test]
    fn test_parse_signal_name_winch() {
        let signal = parse_signal_name("SIGWINCH").unwrap();
        assert_eq!(signal, Signal::SIGWINCH);
    }

    #[test]
    fn test_parse_signal_name_unknown() {
        let result = parse_signal_name("SIGINVALID");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown signal name"));
    }

    #[test]
    fn test_parse_signal_name_empty() {
        let result = parse_signal_name("");
        assert!(result.is_err());
    }
}
