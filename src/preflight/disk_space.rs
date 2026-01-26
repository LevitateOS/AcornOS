//! Disk space check for AcornOS build.
//!
//! Verifies sufficient disk space is available for downloads and build artifacts.

use super::CheckResult;
use distro_builder::process::Cmd;
use std::path::Path;

/// Minimum required disk space in bytes (5 GB).
///
/// Breakdown:
/// - Alpine Extended ISO: ~1 GB
/// - Extracted ISO contents: ~1 GB
/// - Rootfs: ~2 GB
/// - Build artifacts (squashfs, initramfs, ISO): ~1 GB
const MIN_DISK_SPACE_BYTES: u64 = 5 * 1024 * 1024 * 1024;

/// Check that sufficient disk space is available (using shared infrastructure).
pub fn check_disk_space(base_dir: &Path) -> CheckResult {
    // Use df to get available space
    let result = Cmd::new("df")
        .args(["--output=avail", "-B1"]) // Output available bytes
        .arg_path(base_dir)
        .allow_fail()
        .run();

    match result {
        Ok(result) if result.success() => {
            // Skip header line, get first number
            let available = result
                .stdout
                .lines()
                .nth(1)
                .and_then(|line| line.trim().parse::<u64>().ok())
                .unwrap_or(0);

            let available_gb = available as f64 / (1024.0 * 1024.0 * 1024.0);
            let required_gb = MIN_DISK_SPACE_BYTES as f64 / (1024.0 * 1024.0 * 1024.0);

            if available >= MIN_DISK_SPACE_BYTES {
                CheckResult::pass(
                    "Disk space",
                    format!("{:.1} GB available (need {:.1} GB)", available_gb, required_gb),
                )
            } else {
                CheckResult::fail(
                    "Disk space",
                    format!(
                        "Only {:.1} GB available, need {:.1} GB",
                        available_gb, required_gb
                    ),
                    "Free up disk space or use a different build directory",
                )
            }
        }
        _ => CheckResult::fail(
            "Disk space",
            "Failed to check available disk space",
            "Ensure df command is available",
        ),
    }
}

/// Get available disk space in bytes (for programmatic use).
#[allow(dead_code)]
pub fn available_space(path: &Path) -> Option<u64> {
    Cmd::new("df")
        .args(["--output=avail", "-B1"])
        .arg_path(path)
        .allow_fail()
        .run()
        .ok()
        .filter(|r| r.success())
        .and_then(|r| {
            r.stdout
                .lines()
                .nth(1)
                .and_then(|line| line.trim().parse::<u64>().ok())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_check_disk_space_current_dir() {
        let result = check_disk_space(Path::new("."));
        // Should at least be able to check (pass or fail)
        assert!(!result.name.is_empty());
    }

    #[test]
    fn test_available_space() {
        let space = available_space(Path::new("."));
        // Should return Some value for current directory
        assert!(space.is_some());
        assert!(space.unwrap() > 0);
    }

    #[test]
    fn test_min_disk_space_is_5gb() {
        assert_eq!(MIN_DISK_SPACE_BYTES, 5 * 1024 * 1024 * 1024);
    }
}
