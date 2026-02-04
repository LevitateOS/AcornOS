//! Tiny initramfs builder (~5MB).
//!
//! Creates a minimal initramfs containing only:
//! - Static busybox binary (~1MB)
//! - /init script (shell script that mounts EROFS)
//! - Kernel modules for boot
//! - Minimal directory structure
//!
//! # Kernel Modules
//!
//! Modules come from our custom kernel build at `output/staging/lib/modules/`.
//! They use zstd compression (.ko.zst) from the kernel build process.
//!
//! # Boot Flow
//!
//! ```text
//! 1. GRUB loads kernel + this initramfs
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

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use distro_builder::artifact::cpio::build_cpio;
use distro_builder::artifact::filesystem::{atomic_move, create_initramfs_dirs};
use distro_builder::process::Cmd;
use distro_spec::acorn::{
    BOOT_DEVICE_PROBE_ORDER, BOOT_MODULES, CPIO_GZIP_LEVEL, INITRAMFS_BUILD_DIR, INITRAMFS_DIRS,
    INITRAMFS_LIVE_OUTPUT, ISO_LABEL, LIVE_OVERLAY_ISO_PATH, ROOTFS_ISO_PATH,
};

/// Verify SHA256 hash of a file.
fn verify_sha256(file: &Path, expected: &str) -> Result<()> {
    let mut f = fs::File::open(file).with_context(|| format!("cannot open {}", file.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024]; // 1MB chunks

    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let hash = hex::encode(hasher.finalize());
    if hash != expected.to_lowercase() {
        bail!(
            "SHA256 integrity check failed for '{}'\n  expected: {}\n  got:      {}",
            file.display(),
            expected.to_lowercase(),
            hash
        );
    }
    Ok(())
}

// =============================================================================
// Busybox Constants (canonical source: deps/alpine.rhai)
// =============================================================================

const BUSYBOX_URL: &str = "https://busybox.net/downloads/binaries/1.35.0-x86_64-linux-musl/busybox";
const BUSYBOX_SHA256: &str = "6e123e7f3202a8c1e9b1f94d8941580a25135382b99e8d3e34fb858bba311348";
const BUSYBOX_URL_ENV: &str = "BUSYBOX_URL";

/// Get busybox download URL from environment or use default.
fn busybox_url() -> String {
    env::var(BUSYBOX_URL_ENV).unwrap_or_else(|_| BUSYBOX_URL.to_string())
}

/// Commands to symlink from busybox.
const BUSYBOX_COMMANDS: &[&str] = &[
    "sh",
    "mount",
    "umount",
    "mkdir",
    "cat",
    "ls",
    "sleep",
    "switch_root",
    "echo",
    "test",
    "[",
    "grep",
    "sed",
    "ln",
    "rm",
    "cp",
    "mv",
    "chmod",
    "chown",
    "mknod",
    "losetup",
    "mount.loop",
    "insmod",
    "modprobe",
    "xz",
    "gunzip",
    "find",
    "head",
];

/// Build the tiny initramfs.
pub fn build_tiny_initramfs(base_dir: &Path) -> Result<()> {
    println!("=== Building Tiny Initramfs ===\n");

    let output_dir = base_dir.join("output");
    let initramfs_root = output_dir.join(INITRAMFS_BUILD_DIR);
    let output_cpio = output_dir.join(INITRAMFS_LIVE_OUTPUT);

    // Clean previous build
    if initramfs_root.exists() {
        fs::remove_dir_all(&initramfs_root)?;
    }

    // Create minimal directory structure
    create_directory_structure(&initramfs_root)?;

    // Copy/download busybox
    copy_busybox(base_dir, &initramfs_root)?;

    // Copy kernel modules from rootfs
    copy_boot_modules(base_dir, &initramfs_root)?;

    // Create init script from template
    create_init_script(base_dir, &initramfs_root)?;

    // Build cpio archive to a temporary file (using shared infrastructure)
    let temp_cpio = output_dir.join(format!("{}.tmp", INITRAMFS_LIVE_OUTPUT));
    build_cpio(&initramfs_root, &temp_cpio, CPIO_GZIP_LEVEL)?;

    // Verify the temporary artifact is valid
    if !temp_cpio.exists() || fs::metadata(&temp_cpio)?.len() < 1024 {
        bail!("Initramfs build produced invalid or empty file");
    }

    // Atomic move to final destination (with cross-filesystem fallback)
    atomic_move(&temp_cpio, &output_cpio)?;

    let size = fs::metadata(&output_cpio)?.len();
    println!("\n=== Tiny Initramfs Complete ===");
    println!("  Output: {}", output_cpio.display());
    println!("  Size: {} KB", size / 1024);

    Ok(())
}

/// Create minimal directory structure using shared infrastructure.
fn create_directory_structure(root: &Path) -> Result<()> {
    println!("Creating directory structure...");

    // Use shared create_initramfs_dirs() from distro-builder
    // INITRAMFS_DIRS from distro-spec contains standard dirs (bin, dev, proc, sys, etc.)
    // The shared function also handles standard dirs, so we pass INITRAMFS_DIRS as extra
    create_initramfs_dirs(root, INITRAMFS_DIRS)?;

    // Add note about device nodes
    let dev = root.join("dev");
    fs::write(
        dev.join(".note"),
        "# Device nodes are created by gen_init_cpio or devtmpfs\n",
    )?;

    Ok(())
}

/// Download or copy busybox static binary.
fn copy_busybox(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Setting up busybox...");

    let downloads_dir = base_dir.join("downloads");
    let busybox_cache = downloads_dir.join("busybox-static");
    let busybox_dst = initramfs_root.join("bin/busybox");

    // Download if not cached
    if !busybox_cache.exists() {
        let url = busybox_url();
        let is_default_url = env::var(BUSYBOX_URL_ENV).is_err();
        println!("  Downloading static busybox from {}", url);
        fs::create_dir_all(&downloads_dir)?;

        Cmd::new("curl")
            .args(["-L", "-o"])
            .arg_path(&busybox_cache)
            .args(["--progress-bar", &url])
            .error_msg("Failed to download busybox. Install: sudo dnf install curl")
            .run_interactive()?;

        // Verify checksum only for default URL (custom URLs may have different checksums)
        if is_default_url {
            println!("  Verifying checksum...");
            verify_sha256(&busybox_cache, BUSYBOX_SHA256)
                .context("Busybox checksum verification failed")?;
        } else {
            println!("  Skipping checksum (custom URL)");
        }
    }

    // Copy to initramfs
    fs::copy(&busybox_cache, &busybox_dst)?;

    // Make executable
    let mut perms = fs::metadata(&busybox_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&busybox_dst, perms)?;

    // Create symlinks for common commands
    println!("  Creating busybox symlinks...");
    for cmd in BUSYBOX_COMMANDS {
        let link = initramfs_root.join("bin").join(cmd);
        if !link.exists() {
            std::os::unix::fs::symlink("busybox", &link)?;
        }
    }

    println!("  Busybox ready ({} commands)", BUSYBOX_COMMANDS.len());
    Ok(())
}

/// Copy boot kernel modules to the initramfs.
///
/// Modules come from our custom kernel build at `output/staging/lib/modules/`.
/// They use zstd compression (.ko.zst) from the kernel build process.
///
/// # Built-in vs Modular
///
/// Our kernel config compiles most boot-critical drivers as built-in (=y),
/// not as loadable modules (=m). This means they're already in the kernel
/// binary and don't need to be loaded from initramfs.
///
/// We still attempt to copy any modules that DO exist, but missing modules
/// are assumed to be built-in and are not an error.
fn copy_boot_modules(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Copying boot kernel modules...");

    // Modules are installed to output/staging/lib/modules/ by the kernel build
    let modules_path = base_dir.join("output/staging/lib/modules");

    if !modules_path.exists() {
        bail!(
            "No kernel modules found at {}.\n\
             Run 'acornos build kernel' first.",
            modules_path.display()
        );
    }

    // Find the kernel version directory
    let kernel_version = fs::read_dir(&modules_path)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string());

    let Some(kver) = kernel_version else {
        bail!(
            "No kernel version directory found in {}.\n\
             The rootfs may be incomplete.",
            modules_path.display()
        );
    };

    println!("  Kernel version: {}", kver);

    let kmod_src = modules_path.join(&kver);
    let kmod_dst = initramfs_root.join("lib/modules").join(&kver);
    fs::create_dir_all(&kmod_dst)?;

    // Copy each boot module if it exists
    // Missing modules are assumed to be built-in to the kernel (=y in kconfig)
    let mut copied = 0;
    let mut builtin = 0;
    for module_path in BOOT_MODULES {
        // Try to find the module with different extensions
        // Our custom kernel build uses .ko.zst (zstd compression)
        let base_path = module_path
            .trim_end_matches(".ko.zst")
            .trim_end_matches(".ko.xz")
            .trim_end_matches(".ko.gz")
            .trim_end_matches(".ko");

        let mut found = false;
        // .ko.zst first - that's what our custom kernel build produces
        for ext in [".ko.zst", ".ko", ".ko.gz", ".ko.xz"] {
            let src = kmod_src.join(format!("{}{}", base_path, ext));
            if src.exists() {
                let dst = kmod_dst.join(format!("{}{}", base_path, ext));
                fs::create_dir_all(dst.parent().unwrap())?;
                fs::copy(&src, &dst)?;
                copied += 1;
                found = true;
                break;
            }
        }

        if !found {
            // Module not found as .ko file - assume it's built-in to the kernel
            builtin += 1;
        }
    }

    if copied > 0 {
        println!("  Copied {} boot modules", copied);
    }
    if builtin > 0 {
        println!(
            "  {} boot modules are built-in to kernel (no .ko files)",
            builtin
        );
    }

    // Copy modules.dep and other metadata files for depmod
    for meta_file in [
        "modules.dep",
        "modules.dep.bin",
        "modules.alias",
        "modules.alias.bin",
    ] {
        let src = kmod_src.join(meta_file);
        if src.exists() {
            fs::copy(&src, kmod_dst.join(meta_file))?;
        }
    }

    Ok(())
}

/// Create the init script from template.
fn create_init_script(base_dir: &Path, initramfs_root: &Path) -> Result<()> {
    println!("Creating init script from template...");

    let init_content = generate_init_script(base_dir)?;
    let init_dst = initramfs_root.join("init");

    fs::write(&init_dst, &init_content)?;

    // Make executable
    let mut perms = fs::metadata(&init_dst)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_dst, perms)?;

    Ok(())
}

/// Generate init script from template with distro-spec values.
fn generate_init_script(base_dir: &Path) -> Result<String> {
    let template_path = base_dir.join("profile/init_tiny.template");
    let template = fs::read_to_string(&template_path).with_context(|| {
        format!(
            "Failed to read init_tiny.template at {}",
            template_path.display()
        )
    })?;

    // Extract module names from full paths
    // e.g., "kernel/fs/erofs/erofs.ko.gz" -> "erofs"
    let module_names: Vec<&str> = BOOT_MODULES
        .iter()
        .filter_map(|m| m.rsplit('/').next())
        .map(|m| {
            m.trim_end_matches(".ko.xz")
                .trim_end_matches(".ko.gz")
                .trim_end_matches(".ko")
        })
        .collect();

    Ok(template
        .replace("{{ISO_LABEL}}", ISO_LABEL)
        .replace("{{ROOTFS_PATH}}", &format!("/{}", ROOTFS_ISO_PATH))
        .replace("{{BOOT_MODULES}}", &module_names.join(" "))
        .replace("{{BOOT_DEVICES}}", &BOOT_DEVICE_PROBE_ORDER.join(" "))
        .replace(
            "{{LIVE_OVERLAY_PATH}}",
            &format!("/{}", LIVE_OVERLAY_ISO_PATH),
        ))
}
