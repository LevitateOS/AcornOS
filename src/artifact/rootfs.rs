//! EROFS rootfs builder - creates the AcornOS system image.
//!
//! The rootfs (EROFS) serves as BOTH:
//! - Live boot environment (mounted read-only with tmpfs overlay)
//! - Installation source (extracted to disk by recstrap)
//!
//! # Atomicity
//!
//! Uses Gentoo-style "work directory" pattern:
//! - Build into `.work` files (rootfs-staging.work, filesystem.erofs.work)
//! - Only swap to final locations after successful completion
//! - If cancelled mid-build, existing artifacts are preserved

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_builder::process;
use distro_spec::acorn::verification;
use distro_spec::acorn::{
    EROFS_CHUNK_SIZE, EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, ROOTFS_NAME,
};

use crate::component::{build_system, BuildContext};
use distro_builder::alpine::extract::ExtractPaths;

/// Build the EROFS rootfs using the component system.
pub fn build_rootfs(base_dir: &Path) -> Result<()> {
    println!("=== Building AcornOS System Image (EROFS) ===\n");

    check_host_tools()?;

    let paths = ExtractPaths::new(base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);

    // Verify rootfs exists
    if !paths.rootfs.exists() || !paths.rootfs.join("bin").exists() {
        bail!(
            "Rootfs not found at {}.\n\
             Run 'acornos extract' first.",
            paths.rootfs.display()
        );
    }

    // Gentoo-style: separate "work" vs "final" locations
    let work_staging = output_dir.join("rootfs-staging.work");
    let work_output = output_dir.join("filesystem.erofs.work");
    let final_staging = output_dir.join("rootfs-staging");
    let final_output = output_dir.join(ROOTFS_NAME);

    // Clean work directories only (preserve final)
    let _ = fs::remove_dir_all(&work_staging);
    let _ = fs::remove_file(&work_output);
    fs::create_dir_all(&work_staging)?;

    // Build into work directory (may fail — final is preserved)
    let build_result = (|| -> Result<()> {
        let ctx = BuildContext::new(base_dir, &work_staging, "acornos extract")?;
        build_system(&ctx)?;

        // Verify staging before creating EROFS
        verify_staging(&work_staging)?;

        // Build EROFS from staging
        println!("\nCreating EROFS from staging...");
        println!("  Source: {}", work_staging.display());
        println!(
            "  Compression: {} (level {})",
            EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL
        );

        distro_builder::create_erofs(
            &work_staging,
            &work_output,
            EROFS_COMPRESSION,
            EROFS_COMPRESSION_LEVEL,
            EROFS_CHUNK_SIZE,
        )?;
        Ok(())
    })();

    // On failure, clean up work files and propagate error
    if let Err(e) = build_result {
        let _ = fs::remove_dir_all(&work_staging);
        let _ = fs::remove_file(&work_output);
        return Err(e);
    }

    // Atomic swap (only reached if build succeeded)
    println!("\nSwapping work files to final locations...");
    let _ = fs::remove_dir_all(&final_staging);
    let _ = fs::remove_file(&final_output);
    fs::rename(&work_staging, &final_staging)
        .context("Failed to move rootfs-staging.work to rootfs-staging")?;
    fs::rename(&work_output, &final_output)
        .context("Failed to move filesystem.erofs.work to filesystem.erofs")?;

    println!("\n=== EROFS Build Complete ===");
    println!("  Output: {}", final_output.display());
    if let Ok(meta) = fs::metadata(&final_output) {
        println!("  Size: {} MB", meta.len() / 1024 / 1024);
    }

    Ok(())
}

/// Verify the staging directory contains required files before creating EROFS.
fn verify_staging(staging: &Path) -> Result<()> {
    println!("\n  Verifying staging directory...");

    let mut missing = Vec::new();
    let mut passed = 0;

    // Check required binaries
    for bin in verification::REQUIRED_BINARIES {
        if staging.join(bin).exists() {
            passed += 1;
        } else {
            missing.push(*bin);
        }
    }

    // Check FHS directories (may be real dirs or symlinks)
    for dir in verification::REQUIRED_DIRS {
        let p = staging.join(dir);
        if p.exists() || p.is_symlink() {
            passed += 1;
        } else {
            missing.push(*dir);
        }
    }

    // Check config files
    for cfg in verification::REQUIRED_CONFIGS {
        if staging.join(cfg).exists() {
            passed += 1;
        } else {
            missing.push(*cfg);
        }
    }

    // Check init.d directory has services
    let init_d = staging.join(verification::REQUIRED_SERVICE_DIR);
    if init_d.is_dir()
        && fs::read_dir(&init_d)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        passed += 1;
    } else {
        missing.push(verification::REQUIRED_SERVICE_DIR);
    }

    // Check kernel modules directory is non-empty
    let modules = staging.join(verification::KERNEL_MODULES_DIR);
    if modules.is_dir()
        && fs::read_dir(&modules)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        passed += 1;
    } else {
        missing.push(verification::KERNEL_MODULES_DIR);
    }

    let total = passed + missing.len();

    if missing.is_empty() {
        println!("  ✓ Verification PASSED ({}/{} checks)", passed, total);
        Ok(())
    } else {
        println!("  ✗ Verification FAILED ({}/{} checks)", passed, total);
        for item in &missing {
            println!("    ✗ {} - Missing", item);
        }
        bail!(
            "Rootfs verification FAILED: {} missing files.\n\
             The staging directory is incomplete and would produce a broken rootfs.",
            missing.len()
        );
    }
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
