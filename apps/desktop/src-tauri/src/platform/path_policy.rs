use std::path::Path;

const WINDOWS_EXECUTABLE_SUFFIXES: [&str; 4] = [".exe", ".cmd", ".bat", ".com"];

pub fn command_executable_name(path: &Path) -> Option<String> {
    let raw = path.as_os_str().to_str()?.trim_end_matches(['/', '\\']);
    let file_name = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    if file_name.is_empty() {
        return None;
    }

    let lower = file_name.to_ascii_lowercase();
    Some(strip_windows_executable_suffix(&lower).to_string())
}

pub fn is_protected_delete_target(raw_arg: &str) -> bool {
    let target = raw_arg.trim().trim_matches(|ch| ch == '"' || ch == '\'');
    is_protected_unix_delete_target(target) || is_protected_windows_delete_target(target)
}

fn strip_windows_executable_suffix(value: &str) -> &str {
    WINDOWS_EXECUTABLE_SUFFIXES
        .iter()
        .find_map(|suffix| value.strip_suffix(suffix))
        .unwrap_or(value)
}

fn is_protected_unix_delete_target(target: &str) -> bool {
    matches!(target, "/" | "/System" | "/Users")
}

fn is_protected_windows_delete_target(target: &str) -> bool {
    let normalized = target.replace('/', "\\").to_ascii_lowercase();
    let trimmed = normalized.trim_end_matches('\\');

    trimmed == "c:" || trimmed == "c:\\windows" || trimmed.starts_with("c:\\windows\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn normalizes_windows_executable_suffixes() {
        assert_eq!(
            command_executable_name(&PathBuf::from("C:\\Sofvary\\sidecars\\PNPM.CMD")),
            Some("pnpm".to_string())
        );
        assert_eq!(
            command_executable_name(&PathBuf::from("C:/Sofvary/sidecars/node.EXE")),
            Some("node".to_string())
        );
        assert_eq!(
            command_executable_name(&PathBuf::from("/opt/sofvary/pnpm")),
            Some("pnpm".to_string())
        );
    }

    #[test]
    fn detects_platform_protected_delete_targets() {
        for target in [
            "/",
            "/System",
            "/Users",
            "C:\\",
            "C:/Windows",
            "C:\\Windows\\System32",
        ] {
            assert!(
                is_protected_delete_target(target),
                "{target} should be protected"
            );
        }

        assert!(!is_protected_delete_target("generated"));
        assert!(!is_protected_delete_target("C:\\Sofvary\\workspace"));
    }
}
