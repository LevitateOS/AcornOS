//! Squashfs builder - creates the AcornOS system image.
//!
//! The squashfs serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (unsquashed to disk by recstrap)
//!
//! # Architecture
//!
//! ```text
//! Build Flow:
//! downloads/rootfs (Alpine packages)
//!         ↓
//! Component System (FILESYSTEM, BUSYBOX, OPENRC, BRANDING, ...)
//!         ↓
//! output/squashfs-root (staging)
//!         ↓
//! output/filesystem.squashfs
//!
//! ISO Contents:
//! ├── boot/
//! │   ├── vmlinuz              # Alpine LTS kernel
//! │   └── initramfs.img        # Tiny (~5MB) - busybox + mount logic
//! ├── live/
//! │   └── filesystem.squashfs  # Complete system (~200MB)
//! └── EFI/BOOT/
//!     ├── BOOTX64.EFI
//!     └── grub.cfg
//!
//! Live Boot Flow:
//! 1. GRUB loads kernel + tiny initramfs
//! 2. Tiny init mounts ISO by LABEL
//! 3. Mounts filesystem.squashfs read-only via loop device
//! 4. Creates overlay: squashfs (lower) + tmpfs (upper)
//! 5. switch_root to overlay
//! 6. OpenRC boots as PID 1
//! ```

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use distro_builder::process::{self, Cmd};
use distro_spec::acorn::{SQUASHFS_BLOCK_SIZE, SQUASHFS_COMPRESSION, SQUASHFS_NAME};

use crate::component::{build_system, BuildContext};
use crate::extract::ExtractPaths;

/// Build the squashfs using the component system.
///
/// This is the main entry point for building the AcornOS system image.
/// It executes all components in phase order to build the staging directory,
/// then creates a squashfs from it.
///
/// # Flow
///
/// 1. Verify rootfs exists (Alpine packages from `extract`)
/// 2. Create BuildContext pointing to rootfs and staging
/// 3. Execute all components (FILESYSTEM, BUSYBOX, OPENRC, etc.)
/// 4. Create squashfs from staging directory
pub fn build_squashfs(base_dir: &Path) -> Result<()> {
    println!("=== Building AcornOS System Image ===\n");

    check_host_tools()?;

    let paths = ExtractPaths::new(base_dir);
    let output_dir = base_dir.join("output");
    let staging = output_dir.join("squashfs-root");
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
    let ctx = BuildContext::new(base_dir, &staging)?;

    // Execute component system
    build_system(&ctx)?;

    // Work file pattern for atomic builds
    let work_output = output_dir.join("filesystem.squashfs.work");
    let final_output = output_dir.join(SQUASHFS_NAME);

    // Clean work file if it exists
    let _ = fs::remove_file(&work_output);

    // Build squashfs from staging
    println!("\nCreating squashfs from staging...");
    println!("  Source: {}", staging.display());
    println!("  Compression: {}", SQUASHFS_COMPRESSION);
    println!("  Block size: {}", SQUASHFS_BLOCK_SIZE);

    let result = create_squashfs_internal(&staging, &work_output);

    // On failure, clean up work file
    if let Err(e) = result {
        let _ = fs::remove_file(&work_output);
        return Err(e);
    }

    // Atomic rename
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_output, &final_output)?;

    println!("\n=== Squashfs Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Build squashfs directly from rootfs (bypasses component system).
///
/// This is the legacy function that creates a squashfs directly from
/// the extracted Alpine rootfs. Use `build_squashfs()` instead for
/// proper AcornOS branding and configuration.
pub fn build_squashfs_raw(base_dir: &Path) -> Result<()> {
    println!("=== Building Squashfs (Raw Mode) ===\n");

    check_host_tools()?;

    let paths = ExtractPaths::new(base_dir);
    let output_dir = base_dir.join("output");
    fs::create_dir_all(&output_dir)?;

    // Verify rootfs exists
    if !paths.rootfs.exists() || !paths.rootfs.join("bin").exists() {
        bail!(
            "Rootfs not found at {}.\n\
             Run 'acornos extract' first.",
            paths.rootfs.display()
        );
    }

    // Work file pattern for atomic builds
    let work_output = output_dir.join("filesystem.squashfs.work");
    let final_output = output_dir.join(SQUASHFS_NAME);

    // Clean work file if it exists
    let _ = fs::remove_file(&work_output);

    // Build squashfs
    println!("Creating squashfs from rootfs...");
    println!("  Source: {}", paths.rootfs.display());
    println!("  Compression: {}", SQUASHFS_COMPRESSION);
    println!("  Block size: {}", SQUASHFS_BLOCK_SIZE);

    let result = create_squashfs_internal(&paths.rootfs, &work_output);

    // On failure, clean up work file
    if let Err(e) = result {
        let _ = fs::remove_file(&work_output);
        return Err(e);
    }

    // Atomic rename
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_output, &final_output)?;

    println!("\n=== Squashfs Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Check that required host tools are available.
fn check_host_tools() -> Result<()> {
    if !process::exists("mksquashfs") {
        bail!(
            "mksquashfs not found. Install squashfs-tools:\n\
             On Fedora: sudo dnf install squashfs-tools\n\
             On Ubuntu: sudo apt install squashfs-tools"
        );
    }
    Ok(())
}

/// Create a squashfs image from a directory.
fn create_squashfs_internal(source: &Path, output: &Path) -> Result<()> {
    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    Cmd::new("mksquashfs")
        .arg_path(source)
        .arg_path(output)
        .args(["-comp", SQUASHFS_COMPRESSION])
        .args(["-b", SQUASHFS_BLOCK_SIZE])
        .arg("-no-xattrs")
        .arg("-noappend")
        .arg("-all-root")
        .arg("-progress")
        .error_msg("mksquashfs failed. Install squashfs-tools: sudo dnf install squashfs-tools")
        .run_interactive()?;

    // Print size
    let metadata = fs::metadata(output)?;
    println!(
        "Squashfs created: {} MB",
        metadata.len() / 1024 / 1024
    );

    Ok(())
}
