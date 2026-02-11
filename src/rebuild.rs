//! Rebuild detection logic for AcornOS.
//!
//! Uses hash-based caching to skip rebuilding artifacts that haven't changed.
//! This provides faster incremental builds by detecting when inputs change.
//!
//! Kernel theft from LevitateOS is handled by the recipe system (deps/linux.rhai),
//! not by the rebuild detection logic.

use std::path::Path;

use distro_spec::acorn::{INITRAMFS_LIVE_OUTPUT, ISO_FILENAME, ROOTFS_NAME};

use distro_builder::cache;

/// Check if kernel needs to be compiled.
///
/// Checks if the kernel build artifacts exist and if inputs (kconfig) have changed.
/// Theft from LevitateOS is handled by the recipe system, not here.
pub fn kernel_needs_compile(base_dir: &Path) -> bool {
    let our_bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    if !our_bzimage.exists() {
        return true;
    }

    // Check if inputs changed (kconfig)
    let kconfig = base_dir.join("kconfig");
    let hash_file = base_dir.join("output/.kernel-inputs.hash");

    let inputs: Vec<&Path> = vec![&kconfig];

    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &our_bzimage)
}

/// Check if kernel needs to be installed (bzImage exists but vmlinuz doesn't).
pub fn kernel_needs_install(base_dir: &Path) -> bool {
    let bzimage = base_dir.join("output/kernel-build/arch/x86/boot/bzImage");
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");

    if !bzimage.exists() {
        return false; // Can't install what doesn't exist
    }

    if !vmlinuz.exists() {
        return true;
    }

    // Reinstall if bzImage is newer than vmlinuz
    cache::is_newer(&bzimage, &vmlinuz)
}

/// Cache the kernel input hash after a successful build.
pub fn cache_kernel_hash(base_dir: &Path) {
    let kconfig = base_dir.join("kconfig");
    let kernel_source_dir = base_dir
        .join("downloads")
        .join(distro_spec::acorn::KERNEL_SOURCE.source_dir_name());
    let kernel_makefile = kernel_source_dir.join("Makefile");

    let inputs: Vec<&Path> = if kernel_makefile.exists() {
        vec![&kconfig, &kernel_makefile]
    } else {
        vec![&kconfig]
    };

    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.kernel-inputs.hash"), &hash);
    }
}

/// Check if rootfs (EROFS) needs to be rebuilt.
///
/// Uses hash of key input files. Falls back to mtime if hash file missing.
pub fn rootfs_needs_rebuild(base_dir: &Path) -> bool {
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
    let hash_file = base_dir.join("output/.rootfs-inputs.hash");

    if !rootfs.exists() {
        return true;
    }

    // Key files that affect rootfs content
    // For AcornOS, the rootfs comes from Alpine package extraction
    let rootfs_marker = base_dir.join("downloads/rootfs/bin/busybox");
    let extract_module = base_dir.join("src/extract.rs");

    let inputs: Vec<&Path> = vec![&rootfs_marker, &extract_module];
    let current_hash = match cache::hash_files(&inputs) {
        Some(h) => h,
        None => return true,
    };

    cache::needs_rebuild(&current_hash, &hash_file, &rootfs)
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
    let rootfs = base_dir.join("output").join(ROOTFS_NAME);
    let initramfs = base_dir.join("output").join(INITRAMFS_LIVE_OUTPUT);
    // AcornOS builds its own kernel (same as LevitateOS)
    let kernel = base_dir.join("output/staging/boot/vmlinuz");

    if !iso.exists() {
        return true;
    }

    // ISO needs rebuild if any component is missing or newer than the ISO
    !rootfs.exists()
        || !initramfs.exists()
        || !kernel.exists()
        || cache::is_newer(&rootfs, &iso)
        || cache::is_newer(&initramfs, &iso)
        || cache::is_newer(&kernel, &iso)
}

/// Cache the rootfs input hash after a successful build.
pub fn cache_rootfs_hash(base_dir: &Path) {
    let rootfs_marker = base_dir.join("downloads/rootfs/bin/busybox");
    let extract_module = base_dir.join("src/extract.rs");

    let inputs: Vec<&Path> = vec![&rootfs_marker, &extract_module];
    if let Some(hash) = cache::hash_files(&inputs) {
        let _ = cache::write_cached_hash(&base_dir.join("output/.rootfs-inputs.hash"), &hash);
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
