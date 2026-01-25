//! Preflight checks for AcornOS build prerequisites.
//!
//! This module validates that all prerequisites are met BEFORE starting
//! expensive operations like downloading or building.
//!
//! # Checks Performed
//!
//! - **Host tools**: 7z, tar, mksquashfs, xorriso are installed
//! - **Network**: Alpine mirror is reachable
//! - **Disk space**: Sufficient space for downloads and build artifacts
//! - **Cache status**: Reports what's already downloaded
//!
//! # Usage
//!
//! ```rust,ignore
//! use acornos::preflight::PreflightChecker;
//!
//! let checker = PreflightChecker::new(base_dir);
//! let report = checker.run_all()?;
//!
//! if !report.is_ok() {
//!     eprintln!("Preflight checks failed:");
//!     for error in report.errors() {
//!         eprintln!("  - {}", error);
//!     }
//!     std::process::exit(1);
//! }
//! ```

mod disk_space;
mod host_tools;
mod network;

pub use disk_space::check_disk_space;
pub use host_tools::check_host_tools;
pub use network::check_network;

use std::path::{Path, PathBuf};

/// Result of a single preflight check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Name of the check
    pub name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Human-readable message
    pub message: String,
    /// Optional suggestion for fixing the issue
    pub suggestion: Option<String>,
}

impl CheckResult {
    /// Create a passing check result.
    pub fn pass(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a failing check result.
    pub fn fail(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            passed: false,
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    /// Create a warning check result (passes but with a note).
    pub fn warn(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            message: message.into(),
            suggestion: None,
        }
    }
}

/// Comprehensive preflight report.
#[derive(Debug, Default)]
pub struct PreflightReport {
    /// All check results
    pub checks: Vec<CheckResult>,
    /// Cached files found
    pub cache_status: CacheStatus,
}

impl PreflightReport {
    /// Check if all preflight checks passed.
    pub fn is_ok(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    /// Get all failing checks.
    pub fn errors(&self) -> Vec<&CheckResult> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }

    /// Get count of passing checks.
    pub fn passed_count(&self) -> usize {
        self.checks.iter().filter(|c| c.passed).count()
    }

    /// Get total check count.
    pub fn total_count(&self) -> usize {
        self.checks.len()
    }

    /// Print a summary of the preflight checks.
    pub fn print_summary(&self) {
        println!("=== Preflight Check Results ===\n");

        for check in &self.checks {
            let status = if check.passed { "[OK]" } else { "[FAIL]" };
            println!("{} {}: {}", status, check.name, check.message);
            if let Some(suggestion) = &check.suggestion {
                println!("     Suggestion: {}", suggestion);
            }
        }

        println!();
        println!("=== Cache Status ===\n");
        self.cache_status.print();

        println!();
        if self.is_ok() {
            println!("All preflight checks passed ({}/{})", self.passed_count(), self.total_count());
        } else {
            println!(
                "Preflight checks failed: {} of {} passed",
                self.passed_count(),
                self.total_count()
            );
        }
    }
}

/// Status of cached downloads.
#[derive(Debug, Default)]
pub struct CacheStatus {
    /// Alpine ISO is downloaded
    pub has_alpine_iso: bool,
    /// ISO is extracted
    pub has_iso_contents: bool,
    /// apk-tools-static is downloaded and extracted
    pub has_apk_tools: bool,
    /// Rootfs has been created
    pub has_rootfs: bool,
    /// Busybox static binary is cached
    pub has_busybox: bool,
}

impl CacheStatus {
    /// Print cache status.
    pub fn print(&self) {
        let status = |b: bool| if b { "[cached]" } else { "[missing]" };

        println!("{}  Alpine Extended ISO", status(self.has_alpine_iso));
        println!("{}  ISO contents extracted", status(self.has_iso_contents));
        println!("{}  apk-tools-static", status(self.has_apk_tools));
        println!("{}  Rootfs", status(self.has_rootfs));
        println!("{}  Busybox static", status(self.has_busybox));
    }
}

/// Preflight checker for AcornOS build prerequisites.
pub struct PreflightChecker {
    base_dir: PathBuf,
}

impl PreflightChecker {
    /// Create a new preflight checker.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Run all preflight checks and return a comprehensive report.
    pub async fn run_all(&self) -> PreflightReport {
        let mut report = PreflightReport::default();

        // Check host tools
        report.checks.extend(check_host_tools());

        // Check disk space
        report.checks.push(check_disk_space(&self.base_dir));

        // Check network (async)
        report.checks.push(check_network().await);

        // Check cache status
        report.cache_status = self.check_cache_status();

        report
    }

    /// Check what's already cached.
    fn check_cache_status(&self) -> CacheStatus {
        use crate::extract::ExtractPaths;

        let paths = ExtractPaths::new(&self.base_dir);
        let downloads = self.base_dir.join("downloads");

        CacheStatus {
            has_alpine_iso: paths.iso.exists(),
            has_iso_contents: paths.iso_contents.join("apks").exists(),
            has_apk_tools: paths.apk_tools.join("sbin").join("apk.static").exists(),
            has_rootfs: paths.rootfs.join("bin").exists(),
            has_busybox: downloads.join("busybox-static").exists(),
        }
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_pass() {
        let result = CheckResult::pass("test", "passed");
        assert!(result.passed);
        assert!(result.suggestion.is_none());
    }

    #[test]
    fn test_check_result_fail() {
        let result = CheckResult::fail("test", "failed", "fix it");
        assert!(!result.passed);
        assert!(result.suggestion.is_some());
    }

    #[test]
    fn test_preflight_report_is_ok() {
        let mut report = PreflightReport::default();
        assert!(report.is_ok()); // Empty is OK

        report.checks.push(CheckResult::pass("test1", "ok"));
        assert!(report.is_ok());

        report.checks.push(CheckResult::fail("test2", "bad", "fix"));
        assert!(!report.is_ok());
    }
}
