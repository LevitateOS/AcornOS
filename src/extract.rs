//! Alpine Extended ISO download and extraction.
//!
//! This module handles downloading the Alpine Extended ISO (~1GB) and
//! extracting packages to create the AcornOS rootfs.
//!
//! # Architecture
//!
//! Alpine Extended ISO is the Alpine equivalent of Rocky DVD:
//! - Includes Intel/AMD microcode (P0 requirement)
//! - Contains ~200 packages for offline installation
//! - Has `apks/` folder with local package repository
//!
//! # Download Flow
//!
//! ```text
//! 1. Download Alpine Extended ISO
//! 2. Fetch SHA256 checksum
//! 3. Verify checksum
//! 4. Extract ISO with 7z
//! ```
//!
//! # Extract Flow
//!
//! ```text
//! 1. Download apk-tools-static
//! 2. Extract apk.static binary
//! 3. Use apk.static to install packages into rootfs
//! ```

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use distro_builder::process::Cmd;
use distro_spec::acorn::{
    ALPINE_EXTENDED_ISO_FILENAME, ALPINE_EXTENDED_ISO_SHA256_URL, ALPINE_EXTENDED_ISO_SIZE,
    ALPINE_EXTENDED_ISO_URL, ALPINE_ISO_PATH_ENV, ALPINE_VERSION, APK_TOOLS_STATIC_FILENAME,
    APK_TOOLS_STATIC_URL,
};
use leviso_deps::download::{http, verify_sha256, DownloadOptions};

/// Alpine repository URLs (updated to match ALPINE_VERSION).
pub mod repos {
    use super::ALPINE_VERSION;

    /// Main Alpine repository
    pub fn main() -> String {
        format!(
            "https://dl-cdn.alpinelinux.org/alpine/v{}/main",
            ALPINE_VERSION
        )
    }

    /// Community Alpine repository
    pub fn community() -> String {
        format!(
            "https://dl-cdn.alpinelinux.org/alpine/v{}/community",
            ALPINE_VERSION
        )
    }
}

/// Base packages to install when creating the rootfs.
/// These are the minimal set needed for a bootable system.
const BASE_PACKAGES: &[&str] = &[
    // Core Alpine base
    "alpine-base",
    // Init system
    "openrc",
    "openrc-init",
    // Hardware support
    "linux-lts",          // LTS kernel
    "linux-firmware",     // Device firmware
    "intel-ucode",        // Intel microcode
    "amd-ucode",          // AMD microcode
    // Device management (P0)
    "eudev",              // udev-compatible device manager
    "eudev-openrc",       // OpenRC service for eudev
    // Boot
    "grub",
    "grub-efi",
    "efibootmgr",
    // Filesystem
    "e2fsprogs",          // ext4 tools
    "dosfstools",         // FAT tools for EFI
    "util-linux",         // mount, fdisk, etc.
    // Storage & Encryption (P0)
    "cryptsetup",         // LUKS disk encryption
    "lvm2",               // Logical Volume Manager
    "btrfs-progs",        // Btrfs filesystem tools
    "device-mapper",      // Required by cryptsetup/lvm2
    // Network
    "dhcpcd",
    "iproute2",
    "iputils",            // ping
    // Shell and utilities
    "bash",               // bash shell (not just ash)
    "busybox",
    "coreutils",
    // Text editor
    "vim",
    // Hardware info
    "pciutils",           // lspci
    "usbutils",           // lsusb
    // SSH
    "openssh",
];

/// Paths used during download and extraction.
pub struct ExtractPaths {
    /// Downloads directory
    pub downloads: PathBuf,
    /// Path to the Alpine ISO
    pub iso: PathBuf,
    /// Extracted ISO contents
    pub iso_contents: PathBuf,
    /// Rootfs directory
    pub rootfs: PathBuf,
    /// APK tools directory
    pub apk_tools: PathBuf,
}

impl ExtractPaths {
    /// Create paths relative to the base directory.
    pub fn new(base_dir: &Path) -> Self {
        let downloads = base_dir.join("downloads");
        Self {
            iso: downloads.join(ALPINE_EXTENDED_ISO_FILENAME),
            iso_contents: downloads.join("iso-contents"),
            rootfs: downloads.join("rootfs"),
            apk_tools: downloads.join("apk-tools"),
            downloads,
        }
    }
}

/// Check if the Alpine ISO is already downloaded.
pub fn find_existing_iso(base_dir: &Path) -> Option<PathBuf> {
    // First check environment variable
    if let Ok(path) = env::var(ALPINE_ISO_PATH_ENV) {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Then check downloads directory
    let paths = ExtractPaths::new(base_dir);
    if paths.iso.exists() {
        return Some(paths.iso);
    }

    None
}

/// Download the Alpine Extended ISO.
///
/// This downloads the ISO and verifies its SHA256 checksum.
pub async fn download_alpine_iso(base_dir: &Path) -> Result<PathBuf> {
    let paths = ExtractPaths::new(base_dir);
    fs::create_dir_all(&paths.downloads)?;

    // Check if already downloaded
    if let Some(existing) = find_existing_iso(base_dir) {
        println!("Alpine ISO already exists at {}", existing.display());
        return Ok(existing);
    }

    println!("Downloading Alpine Extended ISO...");
    println!("  URL: {}", ALPINE_EXTENDED_ISO_URL);
    println!("  Size: ~{}MB", ALPINE_EXTENDED_ISO_SIZE / 1_000_000);

    // Download the ISO
    let options = DownloadOptions::large_file(ALPINE_EXTENDED_ISO_SIZE);
    http(ALPINE_EXTENDED_ISO_URL, &paths.iso, &options)
        .await
        .context("Failed to download Alpine Extended ISO")?;

    // Fetch and verify checksum
    println!("Verifying checksum...");
    let checksum = fetch_checksum().await?;
    verify_sha256(&paths.iso, &checksum, true).context("ISO checksum verification failed")?;

    println!("Download complete: {}", paths.iso.display());
    Ok(paths.iso)
}

/// Fetch the SHA256 checksum from the Alpine mirrors.
async fn fetch_checksum() -> Result<String> {
    // Download checksum file to temp location
    let temp_dir = std::env::temp_dir();
    let checksum_file = temp_dir.join("alpine-iso.sha256");

    let options = DownloadOptions {
        show_progress: false,
        ..Default::default()
    };

    http(ALPINE_EXTENDED_ISO_SHA256_URL, &checksum_file, &options)
        .await
        .context("Failed to download checksum file")?;

    // Parse checksum file (format: "hash  filename")
    let content = fs::read_to_string(&checksum_file)?;
    let checksum = content
        .split_whitespace()
        .next()
        .context("Checksum file is empty or malformed")?
        .to_string();

    // Clean up
    let _ = fs::remove_file(&checksum_file);

    Ok(checksum)
}

/// Extract the Alpine ISO.
///
/// This extracts the ISO contents using 7z.
pub fn extract_alpine_iso(base_dir: &Path) -> Result<()> {
    let paths = ExtractPaths::new(base_dir);

    // Check ISO exists
    let iso_path = find_existing_iso(base_dir)
        .context("Alpine ISO not found. Run 'acornos download' first.")?;

    // Skip if already extracted
    if paths.iso_contents.exists() && paths.iso_contents.join("apks").exists() {
        println!(
            "ISO already extracted to {}",
            paths.iso_contents.display()
        );
        return Ok(());
    }

    println!("Extracting ISO contents with 7z...");
    fs::create_dir_all(&paths.iso_contents)?;

    Cmd::new("7z")
        .args(["x", "-y"])
        .arg_path(&iso_path)
        .arg(format!("-o{}", paths.iso_contents.display()))
        .error_msg("7z extraction failed. Install: sudo dnf install p7zip-plugins")
        .run_interactive()?;

    // Verify extraction
    let apks_dir = paths.iso_contents.join("apks").join("x86_64");
    if !apks_dir.exists() {
        bail!(
            "ISO extraction incomplete: apks/x86_64/ not found.\n\
             Expected at: {}",
            apks_dir.display()
        );
    }

    println!("ISO extracted successfully");
    Ok(())
}

/// Download apk-tools-static for bootstrapping.
pub async fn download_apk_tools(base_dir: &Path) -> Result<PathBuf> {
    let paths = ExtractPaths::new(base_dir);
    fs::create_dir_all(&paths.apk_tools)?;

    let apk_file = paths.apk_tools.join(APK_TOOLS_STATIC_FILENAME);
    let apk_static = paths.apk_tools.join("sbin").join("apk.static");

    // Skip if already extracted
    if apk_static.exists() {
        println!("apk-tools-static already available at {}", apk_static.display());
        return Ok(apk_static);
    }

    // Download if needed
    if !apk_file.exists() {
        println!("Downloading apk-tools-static...");
        let options = DownloadOptions::default();
        http(APK_TOOLS_STATIC_URL, &apk_file, &options)
            .await
            .context("Failed to download apk-tools-static")?;
    }

    // Extract the APK (it's a tarball)
    println!("Extracting apk-tools-static...");
    Cmd::new("tar")
        .args(["xzf"])
        .arg_path(&apk_file)
        .args(["-C"])
        .arg_path(&paths.apk_tools)
        .error_msg("Failed to extract apk-tools-static")
        .run()?;

    if !apk_static.exists() {
        bail!(
            "apk.static not found after extraction.\n\
             Expected at: {}",
            apk_static.display()
        );
    }

    // Make it executable
    Cmd::new("chmod")
        .args(["+x"])
        .arg_path(&apk_static)
        .run()?;

    println!("apk-tools-static ready at {}", apk_static.display());
    Ok(apk_static)
}

/// Create an Alpine rootfs using apk-tools-static.
///
/// This uses the packages from the extracted ISO to bootstrap a rootfs.
pub fn create_rootfs(base_dir: &Path) -> Result<()> {
    let paths = ExtractPaths::new(base_dir);

    // Check prerequisites
    let apk_static = paths.apk_tools.join("sbin").join("apk.static");
    if !apk_static.exists() {
        bail!(
            "apk-tools-static not found. Run download first.\n\
             Expected at: {}",
            apk_static.display()
        );
    }

    let apks_dir = paths.iso_contents.join("apks").join("x86_64");
    if !apks_dir.exists() {
        bail!(
            "ISO not extracted. Run extract first.\n\
             Expected apks at: {}",
            apks_dir.display()
        );
    }

    // Create rootfs directory
    fs::create_dir_all(&paths.rootfs)?;

    // Create required directories for apk
    for dir in ["etc/apk", "var/cache/apk"] {
        fs::create_dir_all(paths.rootfs.join(dir))?;
    }

    // Set up repository to use local ISO packages
    let repo_path = format!("{}/apks", paths.iso_contents.display());
    let repos_content = format!(
        "{}\n{}\n{}\n",
        repo_path,
        repos::main(),
        repos::community()
    );
    fs::write(paths.rootfs.join("etc/apk/repositories"), &repos_content)?;

    // Initialize the rootfs with base packages
    println!("Installing base packages to rootfs...");
    println!("  Rootfs: {}", paths.rootfs.display());
    println!("  Packages: {} base packages", BASE_PACKAGES.len());

    let mut cmd = Cmd::new(apk_static.to_string_lossy().as_ref());
    cmd = cmd
        .args(["--root"])
        .arg_path(&paths.rootfs)
        .args(["--initdb", "--no-progress", "--allow-untrusted", "add"]);

    for pkg in BASE_PACKAGES {
        cmd = cmd.arg(*pkg);
    }

    cmd.error_msg("Failed to install base packages")
        .run_interactive()?;

    // Verify basic structure
    if !paths.rootfs.join("bin").exists() {
        bail!(
            "Rootfs creation failed: /bin not found.\n\
             Something went wrong during package installation."
        );
    }

    println!("Rootfs created successfully at {}", paths.rootfs.display());
    Ok(())
}

/// Full download workflow: download ISO and apk-tools.
pub async fn cmd_download_impl(base_dir: &Path) -> Result<()> {
    println!("=== AcornOS Download ===");
    println!();

    // Download Alpine ISO
    download_alpine_iso(base_dir).await?;
    println!();

    // Download apk-tools-static
    download_apk_tools(base_dir).await?;
    println!();

    println!("Download complete!");
    println!();
    println!("Next step: acornos extract");

    Ok(())
}

/// Full extract workflow: extract ISO and create rootfs.
pub fn cmd_extract_impl(base_dir: &Path) -> Result<()> {
    println!("=== AcornOS Extract ===");
    println!();

    // Extract ISO
    extract_alpine_iso(base_dir)?;
    println!();

    // Create rootfs
    create_rootfs(base_dir)?;
    println!();

    println!("Extraction complete!");
    println!();
    println!("Rootfs structure:");
    let paths = ExtractPaths::new(base_dir);
    for dir in ["bin", "etc", "lib", "usr", "var"] {
        let path = paths.rootfs.join(dir);
        if path.exists() {
            println!("  /{}/", dir);
        }
    }

    Ok(())
}
