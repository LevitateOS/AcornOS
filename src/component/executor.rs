//! Component executor - interprets Op variants and performs actual operations.
//!
//! This is the single place where all build operations are implemented.
//! ALL operations are required. If something is listed, it must exist.
//! There is no "optional" - this is a daily driver OS, not a toy.

use anyhow::{bail, Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use distro_builder::process::Cmd;

use super::context::BuildContext;
use super::{Component, Op};

/// Execute all operations in a component.
pub fn execute(ctx: &BuildContext, component: &Component) -> Result<()> {
    println!("Installing {}...", component.name);

    for op in component.ops {
        execute_op(ctx, op).with_context(|| format!("in component '{}': {:?}", component.name, op))?;
    }

    Ok(())
}

/// Execute a single operation.
fn execute_op(ctx: &BuildContext, op: &Op) -> Result<()> {
    match op {
        // ─────────────────────────────────────────────────────────────────────
        // Directory operations
        // ─────────────────────────────────────────────────────────────────────
        Op::Dir(path) => {
            fs::create_dir_all(ctx.staging.join(path))?;
        }

        Op::DirMode(path, mode) => {
            let full_path = ctx.staging.join(path);
            fs::create_dir_all(&full_path)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Dirs(paths) => {
            for path in *paths {
                fs::create_dir_all(ctx.staging.join(path))?;
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // File operations
        // ─────────────────────────────────────────────────────────────────────
        Op::WriteFile(path, content) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
        }

        Op::WriteFileMode(path, content, mode) => {
            let full_path = ctx.staging.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
            fs::set_permissions(&full_path, fs::Permissions::from_mode(*mode))?;
        }

        Op::Symlink(link, target) => {
            let link_path = ctx.staging.join(link);
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent)?;
            }
            // Always overwrite existing symlinks - later components take precedence
            // This is CRITICAL for /sbin/init: busybox creates it, OpenRC must override
            if link_path.is_symlink() || link_path.exists() {
                fs::remove_file(&link_path)?;
            }
            std::os::unix::fs::symlink(target, &link_path)?;
        }

        Op::CopyFile(path) => {
            let src = ctx.source.join(path);
            let dst = ctx.staging.join(path);

            if !src.exists() {
                bail!("file not found: {}", src.display());
            }

            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)?;
        }

        Op::CopyTree(path) => {
            copy_tree(&ctx.source.join(path), &ctx.staging.join(path))?;
        }

        // ─────────────────────────────────────────────────────────────────────
        // Binary operations
        // ─────────────────────────────────────────────────────────────────────
        Op::Bin(name) => {
            copy_binary(ctx, name, "usr/bin")?;
        }

        Op::Sbin(name) => {
            copy_binary(ctx, name, "usr/sbin")?;
        }

        Op::Bins(names) => {
            let mut errors = Vec::new();
            for name in *names {
                if let Err(e) = copy_binary(ctx, name, "usr/bin") {
                    errors.push(format!("{}: {}", name, e));
                }
            }
            if !errors.is_empty() {
                bail!("Missing binaries:\n  {}", errors.join("\n  "));
            }
        }

        Op::Sbins(names) => {
            let mut missing = Vec::new();
            for name in *names {
                if let Err(_) = copy_binary(ctx, name, "usr/sbin") {
                    missing.push(*name);
                }
            }
            if !missing.is_empty() {
                bail!("Missing sbin binaries: {}", missing.join(", "));
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // OpenRC operations
        // ─────────────────────────────────────────────────────────────────────
        Op::OpenrcEnable(service, runlevel) => {
            enable_openrc_service(ctx, service, runlevel)?;
        }

        Op::OpenrcScripts(scripts) => {
            for script in *scripts {
                copy_init_script(ctx, script)?;
            }
        }

        Op::OpenrcConf(service, content) => {
            let conf_path = ctx.staging.join("etc/conf.d").join(service);
            if let Some(parent) = conf_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&conf_path, content)?;
        }

        // ─────────────────────────────────────────────────────────────────────
        // User/group operations
        // ─────────────────────────────────────────────────────────────────────
        Op::User {
            name,
            uid,
            gid,
            home,
            shell,
        } => {
            ensure_user(ctx, name, *uid, *gid, home, shell)?;
        }

        Op::Group { name, gid } => {
            ensure_group(ctx, name, *gid)?;
        }

        // ─────────────────────────────────────────────────────────────────────
        // Custom operations
        // ─────────────────────────────────────────────────────────────────────
        Op::Custom(custom_op) => {
            super::custom::execute(ctx, *custom_op)?;
        }
    }

    Ok(())
}

/// Copy a binary with its library dependencies.
///
/// For Alpine/musl, libraries are in /usr/lib (not /usr/lib64).
fn copy_binary(ctx: &BuildContext, name: &str, dest_dir: &str) -> Result<()> {
    // Find the binary in source
    let src_path = ctx.find_binary(name).ok_or_else(|| {
        // Debug: list what's in the source directory
        let usr_bin = ctx.source.join("usr/bin").join(name);
        let bin = ctx.source.join("bin").join(name);
        anyhow::anyhow!(
            "binary not found: {} (checked {} [exists={}] and {} [exists={}])",
            name,
            usr_bin.display(),
            usr_bin.exists(),
            bin.display(),
            bin.exists()
        )
    })?;

    let src = ctx.source.join(&src_path);
    let dst = ctx.staging.join(dest_dir).join(name);

    // Create destination directory
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    // If it's a symlink (busybox applet), recreate the symlink
    if src.is_symlink() {
        let target = fs::read_link(&src)?;
        if dst.exists() || dst.is_symlink() {
            fs::remove_file(&dst)?;
        }
        std::os::unix::fs::symlink(&target, &dst)?;
        return Ok(());
    }

    // Remove existing file/symlink at destination (might be busybox applet)
    if dst.exists() || dst.is_symlink() {
        fs::remove_file(&dst)?;
    }

    // Copy the binary
    fs::copy(&src, &dst).with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
    make_executable(&dst)?;

    // Copy library dependencies (musl-based)
    copy_library_deps(ctx, &src).with_context(|| format!("copying libs for {}", name))?;

    Ok(())
}

/// Copy library dependencies for a binary.
///
/// Uses ldd to find dependencies and copies them to staging.
fn copy_library_deps(ctx: &BuildContext, binary: &Path) -> Result<()> {
    // Run ldd on the binary (using shared infrastructure)
    let result = Cmd::new("ldd")
        .arg_path(binary)
        .allow_fail() // Some binaries (static) don't have deps - that's OK
        .run()
        .context("failed to run ldd")?;

    if !result.success() {
        // Static binary or ldd failed - no deps to copy
        return Ok(());
    }

    let stdout = &result.stdout;

    for line in stdout.lines() {
        // Parse ldd output: "libfoo.so.1 => /usr/lib/libfoo.so.1 (0x...)"
        if let Some(path) = extract_library_path(line) {
            // Only copy libraries from the source rootfs
            let rel_path = path.strip_prefix('/').unwrap_or(&path);
            let src = ctx.source.join(rel_path);
            let dst = ctx.staging.join(rel_path);

            if src.exists() && !dst.exists() {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src, &dst)?;
            }
        }
    }

    Ok(())
}

/// Extract library path from ldd output line.
fn extract_library_path(line: &str) -> Option<String> {
    // Format: "libfoo.so.1 => /usr/lib/libfoo.so.1 (0x...)"
    if let Some(arrow_pos) = line.find("=>") {
        let after_arrow = &line[arrow_pos + 2..];
        let parts: Vec<&str> = after_arrow.trim().split_whitespace().collect();
        if let Some(path) = parts.first() {
            if path.starts_with('/') {
                return Some(path.to_string());
            }
        }
    }
    None
}

/// Make a file executable.
fn make_executable(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Copy a directory tree recursively.
///
/// NOTE: This function logs a warning but continues if the source doesn't exist.
/// This is intentional for optional config directories (like etc/udev/rules.d).
/// For required directories, use a separate validation step.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        // Log but don't fail - some config directories are optional
        println!("  [WARN] copy_tree: source not found: {}", src.display());
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

/// Enable an OpenRC service in a runlevel.
///
/// Creates symlink: /etc/runlevels/<runlevel>/<service> -> /etc/init.d/<service>
fn enable_openrc_service(ctx: &BuildContext, service: &str, runlevel: &str) -> Result<()> {
    let runlevel_dir = ctx.staging.join("etc/runlevels").join(runlevel);
    fs::create_dir_all(&runlevel_dir)?;

    let link = runlevel_dir.join(service);
    let target = format!("/etc/init.d/{}", service);

    if !link.exists() && !link.is_symlink() {
        std::os::unix::fs::symlink(&target, &link)?;
    }

    Ok(())
}

/// Copy an OpenRC init script.
///
/// FAIL FAST: If a script is listed, it must exist. There is no "optional".
fn copy_init_script(ctx: &BuildContext, script: &str) -> Result<()> {
    let src = ctx.source.join("etc/init.d").join(script);
    let dst = ctx.staging.join("etc/init.d").join(script);

    if !src.exists() {
        bail!(
            "OpenRC init script not found: {}\n\
             This script is required. Check that the corresponding package is installed in alpine.rhai.",
            src.display()
        );
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&src, &dst)?;
    make_executable(&dst)?;

    Ok(())
}

/// Ensure a user exists in /etc/passwd.
fn ensure_user(
    ctx: &BuildContext,
    name: &str,
    uid: u32,
    gid: u32,
    home: &str,
    shell: &str,
) -> Result<()> {
    let passwd_path = ctx.staging.join("etc/passwd");

    // Read existing passwd
    let content = if passwd_path.exists() {
        fs::read_to_string(&passwd_path)?
    } else {
        String::new()
    };

    // Check if user already exists
    if content.lines().any(|line| line.starts_with(&format!("{}:", name))) {
        return Ok(());
    }

    // Ensure parent directory exists
    if let Some(parent) = passwd_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Append user
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&passwd_path)?;
    use std::io::Write;
    writeln!(file, "{}:x:{}:{}::{}:{}", name, uid, gid, home, shell)?;

    Ok(())
}

/// Ensure a group exists in /etc/group.
fn ensure_group(ctx: &BuildContext, name: &str, gid: u32) -> Result<()> {
    let group_path = ctx.staging.join("etc/group");

    // Read existing group file
    let content = if group_path.exists() {
        fs::read_to_string(&group_path)?
    } else {
        String::new()
    };

    // Check if group already exists
    if content.lines().any(|line| line.starts_with(&format!("{}:", name))) {
        return Ok(());
    }

    // Ensure parent directory exists
    if let Some(parent) = group_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Append group
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&group_path)?;
    use std::io::Write;
    writeln!(file, "{}:x:{}:", name, gid)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_library_path() {
        // Standard ldd output
        assert_eq!(
            extract_library_path("\tlibc.musl-x86_64.so.1 => /lib/ld-musl-x86_64.so.1 (0x7f...)"),
            Some("/lib/ld-musl-x86_64.so.1".to_string())
        );

        // No path
        assert_eq!(extract_library_path("\tlinux-vdso.so.1"), None);

        // Empty
        assert_eq!(extract_library_path(""), None);
    }
}
