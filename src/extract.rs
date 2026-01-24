//! Alpine APK extraction.
//!
//! This module will handle downloading and extracting Alpine Linux
//! packages to create the AcornOS rootfs.
//!
//! # Status
//!
//! **PLACEHOLDER** - Not yet implemented.
//!
//! # Implementation Options
//!
//! 1. **Use apk-tools binary**
//!    - Requires Alpine Linux or apk-tools-static
//!    - Most reliable, handles dependencies automatically
//!    - Example: `apk --root /mnt add bash vim`
//!
//! 2. **Implement APK parsing in Rust**
//!    - APK files are gzipped tarballs with metadata
//!    - Would need to handle dependencies manually
//!    - More portable but more work
//!
//! 3. **Use Alpine minirootfs**
//!    - Download pre-built minirootfs tarball
//!    - Add packages on top
//!    - Simplest starting point
//!
//! # APK File Format
//!
//! ```text
//! package.apk
//! ├── .PKGINFO      # Package metadata
//! ├── .SIGN.*       # Digital signature
//! └── <files>       # Package contents (tar.gz)
//! ```

use anyhow::Result;
use std::path::Path;

/// Alpine repository URLs.
pub mod repos {
    /// Main Alpine repository
    pub const MAIN: &str = "https://dl-cdn.alpinelinux.org/alpine/v3.19/main";
    /// Community Alpine repository
    pub const COMMUNITY: &str = "https://dl-cdn.alpinelinux.org/alpine/v3.19/community";
}

/// Extract Alpine packages to create a rootfs.
///
/// # Arguments
///
/// * `output_dir` - Directory to extract packages to
///
/// # Status
///
/// **UNIMPLEMENTED**
pub fn extract_alpine_packages(_output_dir: &Path) -> Result<()> {
    unimplemented!(
        "Alpine APK extraction not yet implemented.\n\
        \n\
        Options:\n\
        1. Use apk-tools binary (requires Alpine or apk-tools-static)\n\
        2. Implement APK format parsing in Rust\n\
        3. Use Alpine minirootfs as starting point"
    )
}

/// Download Alpine packages from repository.
///
/// # Arguments
///
/// * `cache_dir` - Directory to cache downloaded packages
/// * `packages` - List of package names to download
///
/// # Status
///
/// **UNIMPLEMENTED**
pub fn download_alpine_packages(_cache_dir: &Path, _packages: &[&str]) -> Result<()> {
    unimplemented!(
        "Alpine package download not yet implemented.\n\
        \n\
        This requires:\n\
        - Repository index parsing (APKINDEX.tar.gz)\n\
        - Dependency resolution\n\
        - Package download with verification"
    )
}

/// Download Alpine minirootfs tarball.
///
/// This is the simplest way to get a working Alpine base.
///
/// # Arguments
///
/// * `output_path` - Path to save the tarball
///
/// # Status
///
/// **UNIMPLEMENTED**
pub fn download_minirootfs(_output_path: &Path) -> Result<()> {
    unimplemented!(
        "Alpine minirootfs download not yet implemented.\n\
        \n\
        URL: https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/\
        alpine-minirootfs-3.19.0-x86_64.tar.gz"
    )
}
