//! Component executor - interprets Op variants and performs actual operations.
//!
//! Delegates to distro-builder shared infrastructure for common operations.
//! Only copy_tree (with its warn-and-continue behavior) and custom ops stay local.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use distro_builder::executor::{binaries, directories, files, openrc, users};
use distro_builder::LicenseTracker;

use super::BuildContext;
use super::{Component, Op};

/// Execute all operations in a component.
pub fn execute(ctx: &BuildContext, component: &Component, tracker: &LicenseTracker) -> Result<()> {
    println!("Installing {}...", component.name);

    for op in component.ops {
        execute_op(ctx, op, tracker)
            .with_context(|| format!("in component '{}': {:?}", component.name, op))?;
    }

    Ok(())
}

/// Execute a single operation.
fn execute_op(ctx: &BuildContext, op: &Op, tracker: &LicenseTracker) -> Result<()> {
    match op {
        // Directory operations
        Op::Dir(path) => directories::handle_dir(&ctx.staging, path)?,
        Op::DirMode(path, mode) => directories::handle_dirmode(&ctx.staging, path, *mode)?,
        Op::Dirs(paths) => directories::handle_dirs(&ctx.staging, paths)?,

        // File operations
        Op::WriteFile(path, content) => files::handle_writefile(&ctx.staging, path, content)?,
        Op::WriteFileMode(path, content, mode) => {
            files::handle_writefilemode(&ctx.staging, path, content, *mode)?
        }
        Op::Symlink(link, target) => files::handle_symlink(&ctx.staging, link, target)?,
        Op::CopyFile(path) => files::handle_copyfile(&ctx.source, &ctx.staging, path)?,
        Op::CopyTree(path) => copy_tree(&ctx.source.join(path), &ctx.staging.join(path))?,

        // Binary operations
        Op::Bin(name) => {
            binaries::copy_binary(&ctx.source, &ctx.staging, name, "usr/bin")?;
            tracker.register_binary(name);
        }
        Op::Sbin(name) => {
            binaries::copy_binary(&ctx.source, &ctx.staging, name, "usr/sbin")?;
            tracker.register_binary(name);
        }
        Op::Bins(names) => {
            let mut errors = Vec::new();
            for name in *names {
                if let Err(e) = binaries::copy_binary(&ctx.source, &ctx.staging, name, "usr/bin") {
                    errors.push(format!("{}: {}", name, e));
                } else {
                    tracker.register_binary(name);
                }
            }
            if !errors.is_empty() {
                bail!("Missing binaries:\n  {}", errors.join("\n  "));
            }
        }
        Op::Sbins(names) => {
            let mut missing = Vec::new();
            for name in *names {
                if binaries::copy_binary(&ctx.source, &ctx.staging, name, "usr/sbin").is_err() {
                    missing.push(*name);
                } else {
                    tracker.register_binary(name);
                }
            }
            if !missing.is_empty() {
                bail!("Missing sbin binaries: {}", missing.join(", "));
            }
        }

        // OpenRC operations
        Op::OpenrcEnable(service, runlevel) => {
            openrc::enable_service(&ctx.staging, service, runlevel)?
        }
        Op::OpenrcScripts(scripts) => {
            for script in *scripts {
                openrc::copy_init_script(&ctx.source, &ctx.staging, script)?;
            }
        }
        Op::OpenrcConf(service, content) => openrc::write_conf(&ctx.staging, service, content)?,

        // User/group operations
        Op::User {
            name,
            uid,
            gid,
            home,
            shell,
        } => users::handle_user(&ctx.source, &ctx.staging, name, *uid, *gid, home, shell)?,
        Op::Group { name, gid } => users::handle_group(&ctx.source, &ctx.staging, name, *gid)?,

        // Custom operations
        Op::Custom(custom_op) => {
            super::custom::execute(ctx, *custom_op, tracker)?;
        }
    }

    Ok(())
}

/// Copy a directory tree recursively.
///
/// NOTE: This function logs a warning but continues if the source doesn't exist.
/// This is intentional for optional config directories (like etc/udev/rules.d).
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
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
