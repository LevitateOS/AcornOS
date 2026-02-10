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

use anyhow::Result;
use std::path::Path;

use distro_spec::acorn::{
    BOOT_DEVICE_PROBE_ORDER, CPIO_GZIP_LEVEL, INITRAMFS_LIVE_OUTPUT, ISO_LABEL,
    LIVE_OVERLAY_ISO_PATH, ROOTFS_ISO_PATH,
};
use recinit::{download_and_cache_busybox, find_kernel_modules_dir, ModulePreset, TinyConfig};

/// Build the tiny initramfs using recinit.
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    let output_dir = base_dir.join("output");

    // Download/cache busybox
    let downloads_dir = base_dir.join("downloads");
    let busybox_path = download_and_cache_busybox(&downloads_dir)?;

    // Find kernel modules directory
    let modules_base = base_dir.join("output/staging/lib/modules");
    let modules_dir = find_kernel_modules_dir(&modules_base)?;

    let config = TinyConfig {
        modules_dir,
        busybox_path,
        template_path: base_dir.join("profile/init_tiny.template"),
        output: output_dir.join(INITRAMFS_LIVE_OUTPUT),
        iso_label: ISO_LABEL.to_string(),
        rootfs_path: ROOTFS_ISO_PATH.to_string(),
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

    recinit::build_tiny_initramfs(&config, true)
}
