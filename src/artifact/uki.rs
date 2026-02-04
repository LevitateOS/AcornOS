//! UKI (Unified Kernel Image) builder for AcornOS.
//!
//! Builds UKIs using the standalone `recuki` crate. UKIs combine kernel + initramfs + cmdline
//! into a single signed PE binary for simplified boot and Secure Boot support.
//!
//! This module provides AcornOS-specific wrappers around recuki, handling:
//! - OS branding (AcornOS name/version in boot menu)
//! - Predefined UKI entries (live, emergency, debug, installed)
//! - Base cmdline construction from distro-spec constants

use anyhow::Result;
use std::path::{Path, PathBuf};

use distro_spec::acorn::{
    ISO_LABEL, OS_ID, OS_NAME, OS_VERSION, SERIAL_CONSOLE, UKI_ENTRIES, UKI_INSTALLED_ENTRIES,
    VGA_CONSOLE,
};
use recuki::UkiConfig;

/// Build a UKI from kernel + initramfs + cmdline.
///
/// Uses `recuki` library which wraps `ukify` from systemd.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image (vmlinuz)
/// * `initramfs` - Path to the initramfs image
/// * `cmdline` - Kernel command line string
/// * `output` - Path for the output .efi file
pub fn build_uki(kernel: &Path, initramfs: &Path, cmdline: &str, output: &Path) -> Result<()> {
    println!("  Building UKI: {}", output.display());

    let config = UkiConfig::new(kernel, initramfs, cmdline, output)
        .with_os_release(OS_NAME, OS_ID, OS_VERSION);

    recuki::build_uki(&config)
}

/// Build UKIs for live ISO boot.
///
/// These UKIs boot from the ISO and mount the EROFS rootfs with live overlay.
/// They are placed in the EFI/Linux/ directory on the ISO for systemd-boot discovery.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image
/// * `initramfs` - Path to the tiny live initramfs (mounts EROFS + overlay)
/// * `output_dir` - Directory to write UKIs to
///
/// # Cmdline
///
/// Uses `root=LABEL=ACORNOS` - the ISO must use this label.
/// systemd-boot auto-discovers UKIs in EFI/Linux/ and presents them as boot menu entries.
///
/// # Returns
///
/// Vector of paths to the created UKI files.
pub fn build_live_ukis(kernel: &Path, initramfs: &Path, output_dir: &Path) -> Result<Vec<PathBuf>> {
    println!("Building UKIs for live ISO boot...");

    // Base cmdline for live boot
    // VGA first, serial last so /dev/console -> serial for testing
    let base_cmdline = format!(
        "root=LABEL={} {} {}",
        ISO_LABEL, VGA_CONSOLE, SERIAL_CONSOLE
    );

    let mut outputs = Vec::new();

    for entry in UKI_ENTRIES {
        let cmdline = if entry.extra_cmdline.is_empty() {
            base_cmdline.clone()
        } else {
            format!("{} {}", base_cmdline, entry.extra_cmdline)
        };

        let output = output_dir.join(entry.filename);
        build_uki(kernel, initramfs, &cmdline, &output)?;
        outputs.push(output);
    }

    println!("  Created {} live UKIs", outputs.len());
    Ok(outputs)
}

/// Build UKIs for installed systems.
///
/// These UKIs use the full initramfs and boot from disk (not ISO).
/// Users copy these to /boot/EFI/Linux/ during installation.
/// systemd-boot auto-discovers UKIs in that directory.
///
/// # Arguments
///
/// * `kernel` - Path to the kernel image
/// * `initramfs` - Path to the full initramfs (not the tiny live one!)
/// * `output_dir` - Directory to write UKIs to
///
/// # Cmdline
///
/// Uses `root=LABEL=root rw` - the user must partition with this label.
/// This can be edited at boot time via systemd-boot if needed.
///
/// # Returns
///
/// Vector of paths to the created UKI files.
pub fn build_installed_ukis(
    kernel: &Path,
    initramfs: &Path,
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    println!("Building UKIs for installed systems...");

    // Base cmdline for installed systems
    // Uses root=LABEL=root - user must label their root partition accordingly
    // Can be edited at boot time if needed (systemd-boot allows editing)
    let base_cmdline = format!("root=LABEL=root rw {} {}", VGA_CONSOLE, SERIAL_CONSOLE);

    let mut outputs = Vec::new();

    for entry in UKI_INSTALLED_ENTRIES {
        let cmdline = if entry.extra_cmdline.is_empty() {
            base_cmdline.clone()
        } else {
            format!("{} {}", base_cmdline, entry.extra_cmdline)
        };

        let output = output_dir.join(entry.filename);
        build_uki(kernel, initramfs, &cmdline, &output)?;
        outputs.push(output);
    }

    println!("  Created {} installed UKIs", outputs.len());
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_cmdline_format() {
        let cmdline = format!(
            "root=LABEL={} {} {}",
            ISO_LABEL, VGA_CONSOLE, SERIAL_CONSOLE
        );

        assert!(cmdline.contains(&format!("root=LABEL={}", ISO_LABEL)));
        assert!(cmdline.contains("console=ttyS0"));
        assert!(cmdline.contains("console=tty0"));
    }

    #[test]
    fn test_uki_entries_defined() {
        // Verify all expected live entries exist
        assert!(UKI_ENTRIES.len() >= 3);
        assert!(UKI_ENTRIES.iter().any(|e| e.filename == "acornos-live.efi"));
        assert!(UKI_ENTRIES
            .iter()
            .any(|e| e.filename == "acornos-emergency.efi"));
        assert!(UKI_ENTRIES
            .iter()
            .any(|e| e.filename == "acornos-debug.efi"));
    }

    #[test]
    fn test_installed_uki_entries_defined() {
        // Verify all expected installed entries exist
        assert!(UKI_INSTALLED_ENTRIES.len() >= 2);
        assert!(UKI_INSTALLED_ENTRIES
            .iter()
            .any(|e| e.filename == "acornos.efi"));
        assert!(UKI_INSTALLED_ENTRIES
            .iter()
            .any(|e| e.filename == "acornos-recovery.efi"));
    }
}
