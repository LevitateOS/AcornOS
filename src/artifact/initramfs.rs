//! Tiny initramfs builder (~5MB).
//!
//! Uses `recinit` to build a minimal busybox-based initramfs for live ISO boot.
//!
//! # Boot Flow
//!
//! ```text
//! 1. systemd-boot loads kernel + this initramfs via UKI
//! 2. Kernel extracts initramfs to rootfs, runs /init
//! 3. /init (busybox sh script):
//!    a. Mount /proc, /sys, /dev
//!    b. Find boot device by LABEL=ACORNOS
//!    c. Mount ISO read-only
//!    d. Mount filesystem.erofs via loop device
//!    e. Create overlay: EROFS (lower) + tmpfs (upper)
//!    f. switch_root to overlay
//! 4. OpenRC (PID 1) takes over
//! ```

use anyhow::{bail, Result};
use std::path::Path;

use distro_spec::acorn::{
    BOOT_DEVICE_PROBE_ORDER, CPIO_GZIP_LEVEL, INITRAMFS_LIVE_OUTPUT, ISO_LABEL,
    LIVE_OVERLAY_ISO_PATH, ROOTFS_ISO_PATH,
};
use recinit::{download_and_cache_busybox, find_kernel_modules_dir, ModulePreset, TinyConfig};

/// Build the tiny initramfs using recinit.
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);

    // Download/cache busybox
    let downloads_dir = base_dir.join("downloads");
    let busybox_path = download_and_cache_busybox(&downloads_dir)?;

    // Find kernel modules directory
    let modules_base = output_dir.join("staging/usr/lib/modules");
    let modules_dir = find_kernel_modules_dir(&modules_base)?;

    let output_path = output_dir.join(INITRAMFS_LIVE_OUTPUT);

    let config = TinyConfig {
        modules_dir,
        busybox_path,
        template_path: base_dir.join("profile/init_tiny.template"),
        output: output_path.clone(),
        iso_label: ISO_LABEL.to_string(),
        rootfs_path: ROOTFS_ISO_PATH.to_string(),
        live_overlay_image_path: Some(LIVE_OVERLAY_ISO_PATH.to_string()),
        live_overlay_path: Some(LIVE_OVERLAY_ISO_PATH.to_string()),
        boot_devices: BOOT_DEVICE_PROBE_ORDER
            .iter()
            .map(|s| s.to_string())
            .collect(),
        module_preset: ModulePreset::Live,
        gzip_level: CPIO_GZIP_LEVEL,
        check_builtin: true,
        extra_template_vars: Vec::new(),
    };

    recinit::build_tiny_initramfs(&config, true)?;

    // Verify the built initramfs
    verify_initramfs(&output_path)?;

    Ok(())
}

/// Verify the initramfs contains essential files.
fn verify_initramfs(path: &Path) -> Result<()> {
    use fsdbg::cpio::CpioReader;

    print!("  Verifying initramfs... ");

    let reader = CpioReader::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open initramfs for verification: {}", e))?;

    let mut missing = Vec::new();

    // Must have /init
    if !reader.exists("init") {
        missing.push("/init");
    }

    // Must have busybox
    if !reader.exists("bin/busybox") {
        missing.push("/bin/busybox");
    }

    // Kernel modules can be built-in, so this check is optional
    // TODO: Only require modules if they're not built-in to the kernel
    // let has_modules = reader.entries().iter().any(|e| {
    //     e.path.starts_with("lib/modules/")
    //         && (e.path.ends_with(".ko") || e.path.ends_with(".ko.zst"))
    // });
    // if !has_modules {
    //     missing.push("lib/modules/**/*.ko (no kernel modules)");
    // }

    if missing.is_empty() {
        println!("OK");
        Ok(())
    } else {
        println!("FAILED");
        for item in &missing {
            println!("    ✗ {} - Missing", item);
        }
        bail!(
            "Initramfs verification FAILED: {} missing items.\n\
             The initramfs is incomplete and will not boot correctly.",
            missing.len()
        );
    }
}
