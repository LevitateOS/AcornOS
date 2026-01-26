//! Kernel building and installation for AcornOS.
//!
//! This module provides AcornOS-specific wrappers around the shared
//! kernel building infrastructure in `distro_builder::build::kernel`.
//!
//! # ============================================================================
//! # IMPORTANT: KERNEL THEFT MODE
//! # ============================================================================
//! #
//! # Currently, AcornOS STEALS the kernel from LevitateOS instead of building
//! # its own. This is because:
//! #
//! #   1. Kernel builds take ~1 HOUR
//! #   2. The kernels are IDENTICAL except for CONFIG_LOCALVERSION
//! #   3. Building the same kernel twice is wasteful
//! #
//! # The leviso kernel is at: leviso/output/kernel-build/
//! #
//! # TODO: When AcornOS needs a genuinely different kernel config (musl-specific
//! # optimizations, OpenRC-specific options, etc.), remove the theft logic and
//! # build our own kernel.
//! #
//! # ============================================================================

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_builder::build::kernel as shared_kernel;
use distro_builder::KernelInstallConfig;

/// AcornOS kernel installation configuration.
///
/// Implements `KernelInstallConfig` using constants from `distro_spec::acorn`.
pub struct AcornKernelConfig;

impl KernelInstallConfig for AcornKernelConfig {
    fn module_install_path(&self) -> &str {
        distro_spec::acorn::MODULE_INSTALL_PATH
    }

    fn kernel_filename(&self) -> &str {
        distro_spec::acorn::KERNEL_FILENAME
    }
}

// ============================================================================
// KERNEL THEFT FUNCTIONS
// ============================================================================
//
// These functions check if LevitateOS has already built a kernel and steal it
// instead of rebuilding from scratch. This saves ~1 hour of build time.
//
// ============================================================================

/// Check if we can steal the kernel from LevitateOS.
///
/// Returns the path to leviso's kernel-build directory if it exists and has
/// a compiled bzImage.
pub fn leviso_kernel_available(base_dir: &Path) -> Option<std::path::PathBuf> {
    // AcornOS is at: workspace/AcornOS/
    // leviso is at:  workspace/leviso/
    let workspace_root = base_dir.parent()?;
    let leviso_kernel_build = workspace_root.join("leviso/output/kernel-build");
    let bzimage = leviso_kernel_build.join("arch/x86/boot/bzImage");

    if bzimage.exists() {
        Some(leviso_kernel_build)
    } else {
        None
    }
}

/// Get the path to leviso's output directory (for module installation).
pub fn leviso_output_dir(base_dir: &Path) -> Option<std::path::PathBuf> {
    let workspace_root = base_dir.parent()?;
    let leviso_output = workspace_root.join("leviso/output");

    if leviso_output.exists() {
        Some(leviso_output)
    } else {
        None
    }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/// Build the kernel from source - OR STEAL IT FROM LEVISO.
///
/// # THEFT MODE
///
/// If LevitateOS has already built a kernel, we STEAL it instead of building
/// our own. This saves ~1 hour of build time since the kernels are identical.
///
/// The theft is logged clearly so you know what's happening.
///
/// # Arguments
/// * `kernel_source` - Path to kernel source tree
/// * `output_dir` - Directory for build artifacts
/// * `base_dir` - AcornOS project root (contains kconfig file)
///
/// # Returns
/// The kernel version string (e.g., "6.12.0-acorn" or "6.12.0-levitate")
pub fn build_kernel(kernel_source: &Path, output_dir: &Path, base_dir: &Path) -> Result<String> {
    // ========================================================================
    // THEFT CHECK: Can we steal from LevitateOS?
    // ========================================================================
    if let Some(leviso_build) = leviso_kernel_available(base_dir) {
        println!();
        println!("  ╔════════════════════════════════════════════════════════════╗");
        println!("  ║  STEALING KERNEL FROM LEVITATEOS                           ║");
        println!("  ║                                                            ║");
        println!("  ║  LevitateOS has already built a kernel. Since our kernels  ║");
        println!("  ║  are identical (for now), we're stealing theirs instead    ║");
        println!("  ║  of wasting ~1 hour rebuilding the same thing.             ║");
        println!("  ║                                                            ║");
        println!("  ║  Source: leviso/output/kernel-build/                       ║");
        println!("  ║                                                            ║");
        println!("  ║  TODO: Build our own when configs actually differ.         ║");
        println!("  ╚════════════════════════════════════════════════════════════╝");
        println!();

        // Create a symlink to leviso's kernel-build instead of copying
        // This way we don't duplicate ~2GB of kernel build artifacts
        let our_kernel_build = output_dir.join("kernel-build");

        // Remove existing directory/symlink if present
        if our_kernel_build.exists() || our_kernel_build.is_symlink() {
            if our_kernel_build.is_symlink() {
                fs::remove_file(&our_kernel_build)?;
            } else {
                fs::remove_dir_all(&our_kernel_build)?;
            }
        }

        // Create parent directory if needed
        fs::create_dir_all(output_dir)?;

        // Symlink to leviso's build
        #[cfg(unix)]
        std::os::unix::fs::symlink(&leviso_build, &our_kernel_build).with_context(|| {
            format!(
                "Failed to symlink {} -> {}",
                our_kernel_build.display(),
                leviso_build.display()
            )
        })?;

        println!("  Created symlink: output/kernel-build -> leviso/output/kernel-build");

        // Return the version from leviso's build
        return shared_kernel::get_kernel_version(&leviso_build);
    }

    // ========================================================================
    // NO THEFT POSSIBLE: Build our own kernel
    // ========================================================================
    println!();
    println!("  (LevitateOS kernel not found - building our own)");
    println!();

    // Clean up any existing kernel-build symlink (e.g., broken symlink from previous theft)
    let our_kernel_build = output_dir.join("kernel-build");
    if our_kernel_build.is_symlink() {
        fs::remove_file(&our_kernel_build).with_context(|| {
            format!(
                "Failed to remove broken symlink at {}",
                our_kernel_build.display()
            )
        })?;
        println!("  Removed stale kernel-build symlink");
    }

    // Read our kconfig
    let kconfig_path = base_dir.join("kconfig");
    if !kconfig_path.exists() {
        bail!(
            "Kernel config not found at {}\nExpected kconfig file in AcornOS project root.",
            kconfig_path.display()
        );
    }
    let kconfig = fs::read_to_string(&kconfig_path)
        .with_context(|| format!("Failed to read {}", kconfig_path.display()))?;

    shared_kernel::build_kernel(kernel_source, output_dir, &kconfig)
}

/// Install kernel and modules to staging directory.
///
/// Uses AcornOS-specific paths from `distro_spec::acorn`.
/// Note: Modules go to /lib/modules/ (not /usr/lib/modules/ like LevitateOS).
///
/// # Arguments
/// * `kernel_source` - Path to kernel source tree
/// * `build_output` - Directory containing kernel-build/
/// * `staging` - Target staging directory
///
/// # Returns
/// The kernel version string
pub fn install_kernel(kernel_source: &Path, build_output: &Path, staging: &Path) -> Result<String> {
    shared_kernel::install_kernel(kernel_source, build_output, staging, &AcornKernelConfig)
}

/// Get the kernel version from the build directory.
///
/// Delegates to shared implementation.
pub fn get_kernel_version(build_dir: &Path) -> Result<String> {
    shared_kernel::get_kernel_version(build_dir)
}
