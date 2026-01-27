//! Build context shared across all build modules.
//!
//! Provides paths needed to build the AcornOS system image.
//!
//! # Key Differences from LevitateOS
//!
//! - Library path: `/usr/lib` (musl), not `/usr/lib64` (glibc)
//! - Source rootfs comes from Alpine APKs, not Rocky RPMs
//! - Busybox provides most binaries via symlinks

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Shared context for all build operations.
///
/// This provides the paths that the executor and custom operations
/// need to copy files from source to staging.
pub struct BuildContext {
    /// Path to the source rootfs (Alpine rootfs with binaries).
    pub source: PathBuf,
    /// Path to the staging directory (where we build the filesystem).
    pub staging: PathBuf,
    /// Base directory of the AcornOS project.
    pub base_dir: PathBuf,
    /// Output directory for build artifacts.
    pub output: PathBuf,
}

impl BuildContext {
    /// Create a new build context.
    ///
    /// # Arguments
    /// * `base_dir` - The AcornOS project root directory
    /// * `staging` - Where to build the filesystem
    ///
    /// # Errors
    ///
    /// Returns an error if the Alpine rootfs doesn't exist.
    /// Run `acornos extract` first to create it.
    pub fn new(base_dir: &Path, staging: &Path) -> Result<Self> {
        let downloads = base_dir.join("downloads");
        let source = downloads.join("rootfs");
        let output = base_dir.join("output");

        if !source.exists() || !source.join("bin").exists() {
            anyhow::bail!(
                "Alpine rootfs not found at {}.\n\
                 Run 'acornos extract' first.",
                source.display()
            );
        }

        Ok(Self {
            source,
            staging: staging.to_path_buf(),
            base_dir: base_dir.to_path_buf(),
            output,
        })
    }

    /// Create a build context from a base directory, creating a staging directory.
    ///
    /// This is a convenience method that creates the staging directory
    /// at `output/rootfs`.
    pub fn from_base_dir(base_dir: &Path) -> Result<Self> {
        let staging = base_dir.join("output").join("rootfs");
        Self::new(base_dir, &staging)
    }

    /// Create a build context for testing without validation.
    ///
    /// This bypasses the check for Alpine rootfs existence.
    /// Only use in tests with mock filesystems.
    #[allow(dead_code)]
    pub fn for_testing(source: &Path, staging: &Path, base_dir: &Path) -> Self {
        Self {
            source: source.to_path_buf(),
            staging: staging.to_path_buf(),
            base_dir: base_dir.to_path_buf(),
            output: base_dir.join("output"),
        }
    }

    /// Get the library path for this distribution.
    ///
    /// AcornOS uses `/usr/lib` (musl), not `/usr/lib64` (glibc).
    pub fn lib_path(&self) -> &'static str {
        "usr/lib"
    }

    /// Check if a file exists in the source rootfs.
    pub fn source_exists(&self, path: &str) -> bool {
        self.source.join(path).exists()
    }

    /// Check if a binary exists in the source rootfs.
    ///
    /// Checks both /usr/bin and /bin locations.
    pub fn binary_exists(&self, name: &str) -> bool {
        self.source.join("usr/bin").join(name).exists()
            || self.source.join("bin").join(name).exists()
    }

    /// Find a binary in the source rootfs.
    ///
    /// Returns the relative path to the binary if found.
    pub fn find_binary(&self, name: &str) -> Option<PathBuf> {
        let candidates = [
            PathBuf::from("usr/bin").join(name),
            PathBuf::from("bin").join(name),
            PathBuf::from("usr/sbin").join(name),
            PathBuf::from("sbin").join(name),
        ];

        for candidate in candidates {
            if self.source.join(&candidate).exists() {
                return Some(candidate);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_lib_path() {
        let dir = tempdir().unwrap();
        let ctx = BuildContext::for_testing(
            dir.path(),
            dir.path().join("staging").as_path(),
            dir.path(),
        );
        assert_eq!(ctx.lib_path(), "usr/lib");
    }

    #[test]
    fn test_source_exists() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(source.join("etc")).unwrap();
        fs::write(source.join("etc/hostname"), "test").unwrap();

        let ctx = BuildContext::for_testing(&source, dir.path(), dir.path());
        assert!(ctx.source_exists("etc/hostname"));
        assert!(!ctx.source_exists("etc/nonexistent"));
    }
}
