//! Alpine APK extraction.
//!
//! # Status: SKELETON
//!
//! Placeholder for Alpine package extraction.
//! This will need to integrate with apk-tools or implement
//! APK format parsing directly.

use anyhow::Result;
use std::path::Path;

/// Extract Alpine packages to create a rootfs.
///
/// # Status: UNIMPLEMENTED
///
/// This requires:
/// - APK repository access
/// - Package dependency resolution
/// - APK format extraction (tar.gz with metadata)
pub fn extract_alpine_packages(_output_dir: &Path) -> Result<()> {
    unimplemented!("Alpine APK extraction not yet implemented.\n\
        \n\
        Options:\n\
        1. Use apk-tools binary (requires Alpine or apk-tools-static)\n\
        2. Implement APK format parsing in Rust\n\
        3. Use Alpine minirootfs as starting point")
}

/// Download Alpine packages from repository.
///
/// # Status: UNIMPLEMENTED
pub fn download_alpine_packages(_cache_dir: &Path, _packages: &[&str]) -> Result<()> {
    unimplemented!("Alpine package download not yet implemented")
}
