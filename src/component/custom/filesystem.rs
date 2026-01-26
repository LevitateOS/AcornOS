//! Filesystem custom operations.
//!
//! Handles FHS symlinks and device manager setup.

use anyhow::Result;
use std::fs;

use super::super::context::BuildContext;

/// Create FHS symlinks for merged /usr.
///
/// Alpine uses merged /usr, so we create symlinks:
/// - /bin -> usr/bin
/// - /sbin -> usr/sbin
/// - /lib -> usr/lib
///
/// Note: AcornOS uses /usr/lib (not /usr/lib64) since musl
/// doesn't use multilib.
pub fn create_fhs_symlinks(ctx: &BuildContext) -> Result<()> {
    let staging = &ctx.staging;

    // Merged /usr symlinks
    let symlinks = [
        ("bin", "usr/bin"),
        ("sbin", "usr/sbin"),
        ("lib", "usr/lib"),
    ];

    for (link, target) in symlinks {
        let link_path = staging.join(link);
        if !link_path.exists() && !link_path.is_symlink() {
            std::os::unix::fs::symlink(target, &link_path)?;
        }
    }

    // /var symlinks
    let var_symlinks = [
        ("var/run", "/run"),
        ("var/lock", "/run/lock"),
    ];

    for (link, target) in var_symlinks {
        let link_path = staging.join(link);
        if !link_path.exists() && !link_path.is_symlink() {
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(target, &link_path)?;
        }
    }

    // Create /run/lock directory
    fs::create_dir_all(staging.join("run/lock"))?;

    Ok(())
}

/// Setup device manager (eudev or mdev).
///
/// AcornOS uses eudev (standalone udev fork) for device management
/// because mdev from busybox is too limited for a daily driver OS.
pub fn setup_device_manager(ctx: &BuildContext) -> Result<()> {
    let staging = &ctx.staging;

    // Create device directories
    fs::create_dir_all(staging.join("dev"))?;
    fs::create_dir_all(staging.join("run/udev"))?;

    // Create essential device nodes (for chroot operations)
    // These are created at boot by eudev, but we need some basics
    // Note: Device nodes can't be created without root, so we just
    // ensure the directories exist.

    // Copy eudev rules if they exist in source
    let rules_src = ctx.source.join("etc/udev/rules.d");
    let rules_dst = staging.join("etc/udev/rules.d");
    if rules_src.exists() {
        copy_tree(&rules_src, &rules_dst)?;
    }

    // Copy default rules
    let lib_rules_src = ctx.source.join("usr/lib/udev/rules.d");
    let lib_rules_dst = staging.join("usr/lib/udev/rules.d");
    if lib_rules_src.exists() {
        fs::create_dir_all(&lib_rules_dst)?;
        copy_tree(&lib_rules_src, &lib_rules_dst)?;
    }

    // Copy udev helpers
    let helpers_src = ctx.source.join("usr/lib/udev");
    let helpers_dst = staging.join("usr/lib/udev");
    if helpers_src.exists() {
        fs::create_dir_all(&helpers_dst)?;
        for entry in fs::read_dir(&helpers_src)? {
            let entry = entry?;
            let path = entry.path();
            // Copy executables and rules directories
            if path.is_file() {
                let dst = helpers_dst.join(entry.file_name());
                fs::copy(&path, &dst)?;
            }
        }
    }

    Ok(())
}

/// Copy a directory tree recursively.
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    if src.is_file() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dst)?;
        return Ok(());
    }

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_symlink() {
            let target = fs::read_link(&src_path)?;
            if dst_path.exists() || dst_path.is_symlink() {
                fs::remove_file(&dst_path)?;
            }
            std::os::unix::fs::symlink(&target, &dst_path)?;
        } else if src_path.is_dir() {
            copy_tree(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
