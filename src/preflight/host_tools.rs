//! Host tool validation for AcornOS build.
//!
//! Checks that required external tools are installed and executable.

use super::CheckResult;
use std::process::Command;

/// Required host tools with their install suggestions.
const REQUIRED_TOOLS: &[(&str, &str, &str)] = &[
    ("7z", "Extract ISO contents", "sudo dnf install p7zip-plugins"),
    ("tar", "Extract APK packages", "sudo dnf install tar"),
    ("mksquashfs", "Build squashfs image", "sudo dnf install squashfs-tools"),
    ("xorriso", "Build bootable ISO", "sudo dnf install xorriso"),
    ("curl", "Download files", "sudo dnf install curl"),
    ("cpio", "Build initramfs", "sudo dnf install cpio"),
];

/// Check that all required host tools are installed.
pub fn check_host_tools() -> Vec<CheckResult> {
    REQUIRED_TOOLS
        .iter()
        .map(|(tool, purpose, install)| check_tool(tool, purpose, install))
        .collect()
}

/// Check a single tool.
fn check_tool(tool: &str, purpose: &str, install_cmd: &str) -> CheckResult {
    match Command::new("which").arg(tool).output() {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = path.trim();
            CheckResult::pass(
                format!("{} tool", tool),
                format!("Found at {} ({})", path, purpose),
            )
        }
        _ => CheckResult::fail(
            format!("{} tool", tool),
            format!("Not found (needed for: {})", purpose),
            install_cmd,
        ),
    }
}

/// Check if a specific tool is available (returns bool for quick checks).
pub fn has_tool(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_tool_existing() {
        // ls should exist on any Unix system
        assert!(has_tool("ls"));
    }

    #[test]
    fn test_has_tool_nonexistent() {
        assert!(!has_tool("definitely_not_a_real_command_12345"));
    }

    #[test]
    fn test_check_host_tools_returns_results() {
        let results = check_host_tools();
        assert_eq!(results.len(), REQUIRED_TOOLS.len());
    }
}
