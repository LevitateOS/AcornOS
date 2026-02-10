//! Builder orchestration - executes all components in phase order.
//!
//! This module provides the main entry point for building the AcornOS
//! system image using the component system.

use anyhow::Result;
use std::fs;

use distro_builder::{LicenseTracker, PackageManager};

use super::definitions::ALL_COMPONENTS;
use super::executor;
use super::BuildContext;

/// Build the complete AcornOS system.
///
/// This executes all components in phase order:
/// 1. Filesystem - FHS directories, merged-usr symlinks
/// 2. Binaries - busybox, additional utilities
/// 3. Init - OpenRC, device manager
/// 4. (MessageBus skipped - no dbus on live)
/// 5. Services - network, SSH, chrony
/// 6. Config - branding, /etc files
/// 7. (Packages skipped - no package manager setup on live)
/// 8. Firmware - WiFi and hardware firmware
/// 9. Final - welcome message, live overlay, installer tools
///
/// # Arguments
///
/// * `ctx` - Build context with source and staging paths
///
/// # Errors
///
/// Returns an error if any component fails to execute.
/// ALL operations are required - there is no "optional".
pub fn build_system(ctx: &BuildContext) -> Result<()> {
    println!("\n=== Building AcornOS System ===\n");

    // Prepare staging directory
    prepare_staging(ctx)?;

    // Track licenses for all binaries we copy
    let tracker = LicenseTracker::new(ctx.source.clone(), PackageManager::Apk);

    // Execute all components
    for component in ALL_COMPONENTS {
        executor::execute(ctx, component, &tracker)?;
    }

    // Copy license files for all redistributed packages
    let license_count = tracker.copy_licenses(&ctx.source, &ctx.staging)?;
    println!("  Copied licenses for {} packages", license_count);

    println!("\n=== System Build Complete ===\n");

    // Print summary
    print_summary(ctx)?;

    Ok(())
}

/// Prepare the staging directory.
///
/// Creates a clean staging directory for the build.
fn prepare_staging(ctx: &BuildContext) -> Result<()> {
    println!("Preparing staging directory: {}", ctx.staging.display());

    // Remove existing staging directory
    if ctx.staging.exists() {
        fs::remove_dir_all(&ctx.staging)?;
    }

    // Create fresh staging directory
    fs::create_dir_all(&ctx.staging)?;

    Ok(())
}

/// Print a summary of the built system.
fn print_summary(ctx: &BuildContext) -> Result<()> {
    // Count files and directories
    let (files, dirs, symlinks) = count_items(&ctx.staging)?;

    println!("Build Summary:");
    println!("  Staging: {}", ctx.staging.display());
    println!("  Files: {}", files);
    println!("  Directories: {}", dirs);
    println!("  Symlinks: {}", symlinks);

    // Calculate total size
    let size = dir_size(&ctx.staging)?;
    println!("  Total size: {:.1} MB", size as f64 / 1024.0 / 1024.0);

    // Verify essential files exist
    let essential_files = [
        "etc/os-release",
        "etc/hostname",
        "etc/passwd",
        "etc/group",
        "usr/bin/busybox",
    ];

    let mut missing = Vec::new();
    for file in &essential_files {
        if !ctx.staging.join(file).exists() {
            missing.push(*file);
        }
    }

    if !missing.is_empty() {
        println!("\n  WARNING: Missing essential files:");
        for file in &missing {
            println!("    - {}", file);
        }
    }

    Ok(())
}

/// Count files, directories, and symlinks in a path.
fn count_items(path: &std::path::Path) -> Result<(usize, usize, usize)> {
    let mut files = 0;
    let mut dirs = 0;
    let mut symlinks = 0;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_symlink() {
                symlinks += 1;
            } else if path.is_dir() {
                dirs += 1;
                let (f, d, s) = count_items(&path)?;
                files += f;
                dirs += d;
                symlinks += s;
            } else {
                files += 1;
            }
        }
    }

    Ok((files, dirs, symlinks))
}

/// Calculate total size of a directory.
fn dir_size(path: &std::path::Path) -> Result<u64> {
    let mut size = 0;

    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_symlink() {
                // Symlinks are tiny, skip
            } else if path.is_dir() {
                size += dir_size(&path)?;
            } else {
                size += fs::metadata(&path)?.len();
            }
        }
    }

    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_count_items() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        fs::write(path.join("file1.txt"), "test").unwrap();
        fs::write(path.join("file2.txt"), "test").unwrap();
        fs::create_dir(path.join("subdir")).unwrap();
        fs::write(path.join("subdir/file3.txt"), "test").unwrap();
        std::os::unix::fs::symlink("file1.txt", path.join("link")).unwrap();

        let (files, dirs, symlinks) = count_items(path).unwrap();
        assert_eq!(files, 3);
        assert_eq!(dirs, 1);
        assert_eq!(symlinks, 1);
    }
}
