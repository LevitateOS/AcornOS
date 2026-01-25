//! Network connectivity check for AcornOS build.
//!
//! Verifies that Alpine mirrors are reachable before starting downloads.

use super::CheckResult;
use distro_spec::acorn::ALPINE_EXTENDED_ISO_URL;

/// Check network connectivity to Alpine mirrors.
///
/// Performs a HEAD request to verify the Alpine CDN is reachable.
pub async fn check_network() -> CheckResult {
    // Use a simple HEAD request via curl to check connectivity
    // This avoids adding reqwest as a dependency
    let result = tokio::process::Command::new("curl")
        .args([
            "--head",           // HEAD request only
            "--silent",         // No progress output
            "--fail",           // Fail on HTTP errors
            "--max-time", "10", // 10 second timeout
            "--output", "/dev/null",
            ALPINE_EXTENDED_ISO_URL,
        ])
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => CheckResult::pass(
            "Network",
            format!("Alpine mirror reachable ({})", mirror_host()),
        ),
        Ok(_) => CheckResult::fail(
            "Network",
            format!("Alpine mirror unreachable ({})", mirror_host()),
            "Check your internet connection or try again later",
        ),
        Err(e) => CheckResult::fail(
            "Network",
            format!("Failed to check network: {}", e),
            "Ensure curl is installed and you have network access",
        ),
    }
}

/// Extract just the host from the Alpine URL for display.
fn mirror_host() -> &'static str {
    "dl-cdn.alpinelinux.org"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_host() {
        assert!(!mirror_host().is_empty());
    }
}
