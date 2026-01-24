//! Rebuild detection logic for AcornOS.
//!
//! Uses hash-based caching to skip rebuilding artifacts that haven't changed.
//! This provides faster incremental builds by detecting when inputs change.

use std::path::Path;

use distro_spec::acorn::{INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, SQUASHFS_NAME};

use crate::cache;

/// Check if squashfs needs to be rebuilt.
///
/// Uses hash of key input files. Falls back to mtime if hash file missing.
pub fn squashfs_needs_rebuild(base_dir: &Path) -> bool {
    let squashfs = base_dir.join("output").join(SQUASHFS_NAME);
    let hash_file = base_dir.join("output/.squashfs-inputs.hash");

    if !squashfs.exists() {
        return true;
    }

    // Key files that affect squashfs content
    // For AcornOS, the rootfs comes from Alpine package extraction
    let rootfs_marker = base_dir.join("downloads/rootfs/bin/busybox");
    let extract_module = base_dir.join("src/extract.rs");

    let inputs: Vec<&Path> = vec![&rootfs_marker, &extract_module];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &squashfs)
}

/// Check if initramfs needs to be rebuilt.
pub fn initramfs_needs_rebuild(base_dir: &Path) -> bool {
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    let hash_file = base_dir.join("output/.initramfs-inputs.hash");
    let init_script = base_dir.join("profile/init_tiny.template");
    let busybox = base_dir.join("downloads/busybox-static");
    let initramfs_module = base_dir.join("src/artifact/initramfs.rs");

    if !initramfs.exists() {
        return true;
    }

    let inputs: Vec<&Path> = vec![&init_script, &busybox, &initramfs_module];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &initramfs)
}

/// Check if ISO needs to be rebuilt.
pub fn iso_needs_rebuild(base_dir: &Path) -> bool {
    let iso = base_dir.join("output").join(ISO_FILENAME);
    let squashfs = base_dir.join("output").join(SQUASHFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    // AcornOS uses kernel from rootfs, not custom built
    let kernel = base_dir.join("downloads/rootfs/boot/vmlinuz-lts");

    if !iso.exists() {
        return true;
    }

    // ISO needs rebuild if any component is missing or newer than the ISO
    !squashfs.exists()
        || !initramfs.exists()
        || !kernel.exists()
        || cache::is_newer(&squashfs, &iso)
        || cache::is_newer(&initramfs, &iso)
        || cache::is_newer(&kernel, &iso)
}

/// Cache the squashfs input hash after a successful build.
pub fn cache_squashfs_hash(base_dir: &Path) {
    let rootfs_marker = base_dir.join("downloads/rootfs/bin/busybox");
    let extract_module = base_dir.join("src/extract.rs");

    let inputs: Vec<&Path> = vec![&rootfs_marker, &extract_module];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.squashfs-inputs.hash"), &hash);
    }
}

/// Cache the initramfs input hash after a successful build.
pub fn cache_initramfs_hash(base_dir: &Path) {
    let init_script = base_dir.join("profile/init_tiny.template");
    let busybox = base_dir.join("downloads/busybox-static");
    let initramfs_module = base_dir.join("src/artifact/initramfs.rs");

    let inputs: Vec<&Path> = vec![&init_script, &busybox, &initramfs_module];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.initramfs-inputs.hash"), &hash);
    }
}
