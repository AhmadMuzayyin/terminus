use crate::CiscoMode;

/// Deteksi mode CLI dari baris prompt terakhir yang diterima.
///
/// Contoh:
/// - `Router>`            -> UserExec
/// - `Router#`             -> Privileged
/// - `Router(config)#`     -> Config
pub fn detect_mode(last_line: &str) -> Option<CiscoMode> {
    let trimmed = last_line.trim_end();
    if trimmed.ends_with("(config)#") || trimmed.contains("(config") && trimmed.ends_with('#') {
        Some(CiscoMode::Config)
    } else if trimmed.ends_with('#') {
        Some(CiscoMode::Privileged)
    } else if trimmed.ends_with('>') {
        Some(CiscoMode::UserExec)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_user_exec() {
        assert_eq!(detect_mode("Router>"), Some(CiscoMode::UserExec));
    }

    #[test]
    fn detects_privileged() {
        assert_eq!(detect_mode("Router#"), Some(CiscoMode::Privileged));
    }

    #[test]
    fn detects_config_mode() {
        assert_eq!(detect_mode("Router(config)#"), Some(CiscoMode::Config));
    }
}
