//! Dependency resolution for AcornOS build.
//!
//! This module implements a 3-tier resolution pattern for external dependencies:
//!
//! 1. **Environment variable**: Override path via `ALPINE_ISO_PATH`, `APK_TOOLS_PATH`
//! 2. **Existing file**: Check if already downloaded in `downloads/`
//! 3. **Download**: Fetch from Alpine mirrors
//!
//! # Usage
//!
//! ```rust,ignore
//! use acornos::deps::AcornDependencyResolver;
//!
//! let resolver = AcornDependencyResolver::new(base_dir);
//!
//! // Check if Alpine ISO is available without downloading
//! if resolver.has_alpine_iso() {
//!     println!("ISO already cached");
//! }
//!
//! // Resolve with download if needed
//! let iso_path = resolver.alpine_iso().await?;
//! ```
//!
//! # Environment Variables
//!
//! - `ALPINE_ISO_PATH`: Path to pre-downloaded Alpine Extended ISO
//! - `APK_TOOLS_PATH`: Path to pre-downloaded apk-tools-static binary
//! - `BUSYBOX_URL`: Override URL for busybox download

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

use distro_spec::acorn::{
    ALPINE_EXTENDED_ISO_FILENAME, ALPINE_EXTENDED_ISO_SHA256_URL, ALPINE_EXTENDED_ISO_SIZE,
    ALPINE_EXTENDED_ISO_URL, ALPINE_ISO_PATH_ENV, APK_TOOLS_PATH_ENV, APK_TOOLS_STATIC_FILENAME,
    APK_TOOLS_STATIC_SHA256, APK_TOOLS_STATIC_URL, BUSYBOX_SHA256, BUSYBOX_URL, BUSYBOX_URL_ENV,
};
use leviso_deps::download::{http, verify_sha256, DownloadOptions};

use crate::preflight::PreflightChecker;

/// Dependency resolver for AcornOS external dependencies.
///
/// Implements a 3-tier resolution pattern:
/// 1. Environment variable override
/// 2. Existing file in downloads directory
/// 3. Download from Alpine mirrors
pub struct AcornDependencyResolver {
    /// Base directory for downloads
    base_dir: PathBuf,
    /// Downloads directory
    downloads_dir: PathBuf,
}

impl AcornDependencyResolver {
    /// Create a new dependency resolver.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        let downloads_dir = base_dir.join("downloads");
        Self {
            base_dir,
            downloads_dir,
        }
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the downloads directory.
    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    // =========================================================================
    // Alpine ISO Resolution
    // =========================================================================

    /// Check if Alpine ISO is available (without downloading).
    pub fn has_alpine_iso(&self) -> bool {
        self.find_alpine_iso().is_some()
    }

    /// Find Alpine ISO path using 3-tier resolution (no download).
    pub fn find_alpine_iso(&self) -> Option<PathBuf> {
        // Tier 1: Environment variable
        if let Ok(path) = env::var(ALPINE_ISO_PATH_ENV) {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // Tier 2: Existing file in downloads
        let cached = self.downloads_dir.join(ALPINE_EXTENDED_ISO_FILENAME);
        if cached.exists() {
            return Some(cached);
        }

        None
    }

    /// Resolve Alpine ISO path, downloading if necessary.
    pub async fn alpine_iso(&self) -> Result<PathBuf> {
        // Try to find existing
        if let Some(path) = self.find_alpine_iso() {
            println!("Alpine ISO: {} (cached)", path.display());
            return Ok(path);
        }

        // Tier 3: Download
        let dest = self.downloads_dir.join(ALPINE_EXTENDED_ISO_FILENAME);
        std::fs::create_dir_all(&self.downloads_dir)?;

        println!("Downloading Alpine Extended ISO...");
        println!("  URL: {}", ALPINE_EXTENDED_ISO_URL);
        println!("  Size: ~{}MB", ALPINE_EXTENDED_ISO_SIZE / 1_000_000);

        let options = DownloadOptions::large_file(ALPINE_EXTENDED_ISO_SIZE);
        http(ALPINE_EXTENDED_ISO_URL, &dest, &options)
            .await
            .context("Failed to download Alpine Extended ISO")?;

        // Verify checksum
        println!("Verifying checksum...");
        let checksum = self.fetch_iso_checksum().await?;
        verify_sha256(&dest, &checksum, true).context("ISO checksum verification failed")?;

        println!("Alpine ISO: {} (downloaded)", dest.display());
        Ok(dest)
    }

    /// Fetch ISO checksum from Alpine mirrors.
    async fn fetch_iso_checksum(&self) -> Result<String> {
        let temp_dir = std::env::temp_dir();
        let checksum_file = temp_dir.join("alpine-iso.sha256");

        let options = DownloadOptions {
            show_progress: false,
            ..Default::default()
        };

        http(ALPINE_EXTENDED_ISO_SHA256_URL, &checksum_file, &options)
            .await
            .context("Failed to download checksum file")?;

        let content = std::fs::read_to_string(&checksum_file)?;
        let checksum = content
            .split_whitespace()
            .next()
            .context("Checksum file is empty or malformed")?
            .to_string();

        let _ = std::fs::remove_file(&checksum_file);
        Ok(checksum)
    }

    // =========================================================================
    // apk-tools-static Resolution
    // =========================================================================

    /// Check if apk-tools-static is available (without downloading).
    pub fn has_apk_tools(&self) -> bool {
        self.find_apk_tools().is_some()
    }

    /// Find apk-tools-static path using 3-tier resolution (no download).
    pub fn find_apk_tools(&self) -> Option<PathBuf> {
        // Tier 1: Environment variable
        if let Ok(path) = env::var(APK_TOOLS_PATH_ENV) {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }

        // Tier 2: Existing extracted binary
        let apk_static = self
            .downloads_dir
            .join("apk-tools")
            .join("sbin")
            .join("apk.static");
        if apk_static.exists() {
            return Some(apk_static);
        }

        None
    }

    /// Resolve apk-tools-static path, downloading and extracting if necessary.
    pub async fn apk_tools(&self) -> Result<PathBuf> {
        // Try to find existing
        if let Some(path) = self.find_apk_tools() {
            println!("apk-tools-static: {} (cached)", path.display());
            return Ok(path);
        }

        // Tier 3: Download and extract
        let apk_tools_dir = self.downloads_dir.join("apk-tools");
        std::fs::create_dir_all(&apk_tools_dir)?;

        let apk_file = apk_tools_dir.join(APK_TOOLS_STATIC_FILENAME);
        let apk_static = apk_tools_dir.join("sbin").join("apk.static");

        // Download if needed
        if !apk_file.exists() {
            println!("Downloading apk-tools-static...");
            let options = DownloadOptions::default();
            http(APK_TOOLS_STATIC_URL, &apk_file, &options)
                .await
                .context("Failed to download apk-tools-static")?;

            // Verify checksum
            println!("Verifying checksum...");
            verify_sha256(&apk_file, APK_TOOLS_STATIC_SHA256, false)
                .context("apk-tools-static checksum verification failed")?;
        }

        // Extract
        println!("Extracting apk-tools-static...");
        distro_builder::process::Cmd::new("tar")
            .args(["xzf"])
            .arg_path(&apk_file)
            .args(["-C"])
            .arg_path(&apk_tools_dir)
            .error_msg("Failed to extract apk-tools-static")
            .run()?;

        if !apk_static.exists() {
            anyhow::bail!(
                "apk.static not found after extraction.\n\
                 Expected at: {}",
                apk_static.display()
            );
        }

        // Make executable
        distro_builder::process::Cmd::new("chmod")
            .args(["+x"])
            .arg_path(&apk_static)
            .run()?;

        println!("apk-tools-static: {} (downloaded)", apk_static.display());
        Ok(apk_static)
    }

    // =========================================================================
    // Busybox Resolution
    // =========================================================================

    /// Check if busybox is available (without downloading).
    pub fn has_busybox(&self) -> bool {
        self.downloads_dir.join("busybox-static").exists()
    }

    /// Resolve busybox path, downloading if necessary.
    pub async fn busybox(&self) -> Result<PathBuf> {
        let dest = self.downloads_dir.join("busybox-static");

        if dest.exists() {
            println!("Busybox: {} (cached)", dest.display());
            return Ok(dest);
        }

        std::fs::create_dir_all(&self.downloads_dir)?;

        let url = env::var(BUSYBOX_URL_ENV).unwrap_or_else(|_| BUSYBOX_URL.to_string());
        let is_default_url = env::var(BUSYBOX_URL_ENV).is_err();

        println!("Downloading static busybox...");
        println!("  URL: {}", url);

        distro_builder::process::Cmd::new("curl")
            .args(["-L", "-o"])
            .arg_path(&dest)
            .args(["--progress-bar", &url])
            .error_msg("Failed to download busybox")
            .run_interactive()?;

        // Verify checksum only for default URL
        if is_default_url {
            println!("Verifying checksum...");
            verify_sha256(&dest, BUSYBOX_SHA256, false)
                .context("Busybox checksum verification failed")?;
        } else {
            println!("Skipping checksum (custom URL)");
        }

        println!("Busybox: {} (downloaded)", dest.display());
        Ok(dest)
    }

    // =========================================================================
    // Preflight Integration
    // =========================================================================

    /// Run preflight checks for this resolver.
    pub async fn preflight_check(&self) -> crate::preflight::PreflightReport {
        let checker = PreflightChecker::new(&self.base_dir);
        checker.run_all().await
    }

    /// Print dependency status.
    pub fn print_status(&self) {
        println!("=== Dependency Status ===\n");

        let status = |available: bool| {
            if available {
                "[available]"
            } else {
                "[missing]"
            }
        };

        println!(
            "{}  Alpine Extended ISO",
            status(self.has_alpine_iso())
        );
        if let Some(path) = self.find_alpine_iso() {
            println!("           {}", path.display());
        }

        println!(
            "{}  apk-tools-static",
            status(self.has_apk_tools())
        );
        if let Some(path) = self.find_apk_tools() {
            println!("           {}", path.display());
        }

        println!(
            "{}  Busybox static",
            status(self.has_busybox())
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_resolver() {
        let dir = tempdir().unwrap();
        let resolver = AcornDependencyResolver::new(dir.path());
        assert_eq!(resolver.base_dir(), dir.path());
    }

    #[test]
    fn test_downloads_dir() {
        let dir = tempdir().unwrap();
        let resolver = AcornDependencyResolver::new(dir.path());
        assert_eq!(resolver.downloads_dir(), dir.path().join("downloads"));
    }

    #[test]
    fn test_has_alpine_iso_empty() {
        let dir = tempdir().unwrap();
        let resolver = AcornDependencyResolver::new(dir.path());
        assert!(!resolver.has_alpine_iso());
    }

    #[test]
    fn test_has_apk_tools_empty() {
        let dir = tempdir().unwrap();
        let resolver = AcornDependencyResolver::new(dir.path());
        assert!(!resolver.has_apk_tools());
    }

    #[test]
    fn test_has_busybox_empty() {
        let dir = tempdir().unwrap();
        let resolver = AcornDependencyResolver::new(dir.path());
        assert!(!resolver.has_busybox());
    }
}
