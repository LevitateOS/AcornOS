//! ISO creation - builds bootable AcornOS ISO.
//!
//! Uses `reciso` for ISO creation with systemd-boot + UKIs.
//! Uses shared live overlay from `distro-builder`.
//!
//! Boot flow:
//! 1. systemd-boot discovers UKIs in EFI/Linux/
//! 2. UKI loads kernel + tiny initramfs
//! 3. init_tiny mounts EROFS as lower layer
//! 4. init_tiny mounts /live/overlay from ISO as middle layer
//! 5. init_tiny mounts tmpfs as upper layer (for writes)
//! 6. switch_root -> OpenRC

use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::path::Path;

use distro_builder::artifact::live_overlay::{
    create_openrc_live_overlay, InittabVariant, LiveOverlayConfig,
};
use distro_spec::acorn::{
    INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ISO_LABEL, OS_ID, OS_NAME, OS_VERSION, ROOTFS_NAME,
    UKI_ENTRIES,
};

/// Create ISO using reciso with systemd-boot + UKIs.
pub fn create_iso(base_dir: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");
    let kernel = output_dir.join("staging/boot/vmlinuz");
    let initramfs = output_dir.join(INITRAMFS_LIVE_OUTPUT);
    let rootfs = output_dir.join(ROOTFS_NAME);
    let label = env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string());
    let iso_output = output_dir.join(ISO_FILENAME);
    let iso_tmp = output_dir.join(format!("{}.tmp", ISO_FILENAME));

    println!("=== Building AcornOS ISO ===\n");

    // Validate inputs
    if !rootfs.exists() {
        bail!(
            "EROFS rootfs not found at {}.\nRun 'acornos build rootfs' first.",
            rootfs.display()
        );
    }
    if !initramfs.exists() {
        bail!(
            "Live initramfs not found at {}.\nRun 'acornos initramfs' first.",
            initramfs.display()
        );
    }
    if !kernel.exists() {
        bail!(
            "Kernel not found at {}.\nRun 'acornos build kernel' first.",
            kernel.display()
        );
    }

    // Create live overlay
    create_live_overlay(&output_dir)?;

    // Build reciso config — systemd-boot + UKIs (write to .tmp for atomicity)
    let mut config = reciso::IsoConfig::new(&kernel, &initramfs, &rootfs, &label, &iso_tmp)
        .with_os_release(OS_NAME, OS_ID, OS_VERSION)
        .with_overlay(output_dir.join("live-overlay"));

    // Add UKI entries from distro-spec
    for entry in UKI_ENTRIES {
        config.ukis.push(reciso::UkiSource::Build {
            name: entry.name.to_string(),
            extra_cmdline: entry.extra_cmdline.to_string(),
            filename: entry.filename.to_string(),
        });
    }

    reciso::create_iso(&config)?;

    // Atomic rename to final destination
    fs::rename(&iso_tmp, &iso_output)?;

    // Verify ISO contents
    verify_iso(&iso_output)?;

    print_iso_summary(&iso_output);
    Ok(())
}

/// Verify ISO contains required boot components.
fn verify_iso(path: &Path) -> Result<()> {
    use fsdbg::iso::IsoReader;

    print!("  Verifying ISO... ");

    let reader = match IsoReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("isoinfo not found") || err_str.contains("isoinfo") {
                println!("SKIPPED (isoinfo not available)");
                return Ok(());
            }
            println!("FAILED");
            bail!("Failed to open ISO: {}", e);
        }
    };

    let required = [
        "/EFI/BOOT/BOOTX64.EFI",
        "/live/filesystem.erofs",
        "/live/overlay",
    ];

    let mut missing = Vec::new();
    for item in &required {
        if !reader.exists(item) {
            missing.push(*item);
        }
    }

    // Check at least one UKI exists in EFI/Linux/
    let has_uki = reader
        .entries()
        .iter()
        .any(|e| e.path.starts_with("/EFI/Linux/") && e.path.ends_with(".efi"));
    if !has_uki {
        missing.push("EFI/Linux/*.efi (no UKI found)");
    }

    if missing.is_empty() {
        println!("OK");
        Ok(())
    } else {
        println!("FAILED");
        for item in &missing {
            println!("    ✗ {} - Missing", item);
        }
        bail!(
            "ISO verification failed: {} items missing. The ISO will not boot correctly.",
            missing.len()
        );
    }
}

/// Create live overlay using shared infrastructure.
fn create_live_overlay(output_dir: &Path) -> Result<()> {
    let base_dir = output_dir.parent().unwrap_or(Path::new("."));
    let profile_overlay = base_dir.join("profile/live-overlay");

    let config = LiveOverlayConfig {
        os_name: OS_NAME,
        inittab: InittabVariant::DesktopWithSerial,
        profile_overlay: if profile_overlay.exists() {
            Some(profile_overlay.as_path())
        } else {
            None
        },
    };

    create_openrc_live_overlay(output_dir, &config)?;
    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== AcornOS ISO Created ===");
    println!("  Output: {}", iso_output.display());
    match fs::metadata(iso_output) {
        Ok(meta) => {
            println!("  Size: {} MB", meta.len() / 1024 / 1024);
        }
        Err(e) => {
            eprintln!("  [WARN] Could not read ISO size: {}", e);
        }
    }
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}
