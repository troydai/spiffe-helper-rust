use anyhow::{anyhow, Context, Result};
pub use nix::sys::signal::Signal;
use nix::unistd::Pid;
use std::fs;
use std::path::Path;

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

/// Send a signal to a process identified by PID
pub fn send_signal(pid: i32, signal: Signal) -> Result<()> {
    nix::sys::signal::kill(Pid::from_raw(pid), signal)
        .with_context(|| format!("Failed to send signal {:?} to process {}", signal, pid))
}

/// Read a PID from a file
pub fn read_pid_from_file(path: &Path) -> Result<i32> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read PID file: {}", path.display()))?;

    content
        .trim()
        .parse::<i32>()
        .with_context(|| format!("Failed to parse PID from file: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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

    #[test]
    fn test_read_pid_from_file() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        writeln!(tmp_file, "12345").unwrap();

        let pid = read_pid_from_file(tmp_file.path()).unwrap();
        assert_eq!(pid, 12345);
    }

    #[test]
    fn test_read_pid_from_file_with_whitespace() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        writeln!(tmp_file, "  67890  ").unwrap();

        let pid = read_pid_from_file(tmp_file.path()).unwrap();
        assert_eq!(pid, 67890);
    }

    #[test]
    fn test_read_pid_from_file_invalid() {
        let mut tmp_file = NamedTempFile::new().unwrap();
        writeln!(tmp_file, "not-a-pid").unwrap();

        let result = read_pid_from_file(tmp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_send_signal_to_self() {
        // We can't easily test if signal was received without complex setup,
        // but we can test that the call succeeds for SIGWINCH (which is harmless)
        let pid = nix::unistd::getpid();
        let result = send_signal(pid.as_raw(), Signal::SIGWINCH);
        assert!(result.is_ok());
    }
}
