//! EROFS rootfs builder - creates the AcornOS system image.
//!
//! The rootfs (EROFS) serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (extracted to disk by recstrap)
//!
//! # EROFS vs Squashfs
//!
//! AcornOS uses EROFS (Enhanced Read-Only File System) because:
//! - Better random-access performance (no linear directory search)
//! - Fixed 4KB output blocks (better disk I/O alignment)
//! - Lower memory overhead during decompression
//! - Used by Fedora 42+, RHEL 10, Android
//! - Shared implementation with LevitateOS (no code duplication)
//!
//! # Architecture
//!
//! ```text
//! Build Flow:
//! downloads/rootfs (Alpine packages)
//!         |
//! Component System (FILESYSTEM, BUSYBOX, OPENRC, BRANDING, ...)
//!         |
//! output/rootfs-staging (staging)
//!         |
//! output/filesystem.erofs
//!
//! ISO Contents:
//! +-- boot/
//! |   +-- vmlinuz              # Alpine LTS kernel
//! |   +-- initramfs.img        # Tiny (~5MB) - busybox + mount logic
//! +-- live/
//! |   +-- filesystem.erofs     # Complete system (~200MB)
//! +-- EFI/BOOT/
//!     +-- BOOTX64.EFI
//!     +-- grub.cfg
//!
//! Live Boot Flow:
//! 1. GRUB loads kernel + tiny initramfs
//! 2. Tiny init mounts ISO by LABEL
//! 3. Mounts filesystem.erofs read-only via loop device
//! 4. Creates overlay: erofs (lower) + tmpfs (upper)
//! 5. switch_root to overlay
//! 6. OpenRC boots as PID 1
//! ```
//!
//! # Implementation
//!
//! The actual EROFS building is done by `distro_builder::create_erofs`.
//! This module provides AcornOS-specific orchestration (staging, atomicity).

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use distro_builder::process;
use distro_spec::acorn::{
    EROFS_CHUNK_SIZE, EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, ROOTFS_NAME,
};

use crate::component::{build_system, BuildContext};
use distro_builder::alpine::extract::ExtractPaths;

/// Build the EROFS rootfs using the component system.
///
/// This is the main entry point for building the AcornOS system image.
/// It executes all components in phase order to build the staging directory,
/// then creates an EROFS image from it.
///
/// # Flow
///
/// 1. Verify rootfs exists (Alpine packages from `extract`)
/// 2. Create BuildContext pointing to rootfs and staging
/// 3. Execute all components (FILESYSTEM, BUSYBOX, OPENRC, etc.)
/// 4. Create EROFS from staging directory
pub fn build_rootfs(base_dir: &Path) -> Result<()> {
    println!("=== Building AcornOS System Image (EROFS) ===\n");

    check_host_tools()?;

    let paths = ExtractPaths::new(base_dir);
    let output_dir = base_dir.join("output");
    let staging = output_dir.join("rootfs-staging");
    fs::create_dir_all(&output_dir)?;

    // Verify rootfs exists
    if !paths.rootfs.exists() || !paths.rootfs.join("bin").exists() {
        bail!(
            "Rootfs not found at {}.\n\
             Run 'acornos extract' first.",
            paths.rootfs.display()
        );
    }

    // Create build context
    let ctx = BuildContext::new(base_dir, &staging, "acornos extract")?;

    // Execute component system
    build_system(&ctx)?;

    // Work file pattern for atomic builds
    let work_output = output_dir.join("filesystem.erofs.work");
    let final_output = output_dir.join(ROOTFS_NAME);

    // Clean work file if it exists
    let _ = fs::remove_file(&work_output);

    // Build EROFS from staging
    println!("\nCreating EROFS from staging...");
    println!("  Source: {}", staging.display());
    println!(
        "  Compression: {} (level {})",
        EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL
    );

    let result = distro_builder::create_erofs(
        &staging,
        &work_output,
        EROFS_COMPRESSION,
        EROFS_COMPRESSION_LEVEL,
        EROFS_CHUNK_SIZE,
    );

    // On failure, clean up work file
    if let Err(e) = result {
        let _ = fs::remove_file(&work_output);
        return Err(e);
    }

    // Atomic rename
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_output, &final_output)?;

    println!("\n=== EROFS Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Check that required host tools are available.
fn check_host_tools() -> Result<()> {
    if !process::exists("mkfs.erofs") {
        bail!(
            "mkfs.erofs not found. Install erofs-utils:\n\
             On Fedora: sudo dnf install erofs-utils\n\
             On Ubuntu: sudo apt install erofs-utils\n\
             On Arch: sudo pacman -S erofs-utils\n\
             \n\
             NOTE: erofs-utils 1.5+ required for lz4hc compression."
        );
    }
    Ok(())
}
