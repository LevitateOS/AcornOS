//! Alpine Linux dependency via recipe.

use super::{find_recipe, run_recipe_json};
use anyhow::{bail, Result};
use distro_builder::process::ensure_exists;
use std::path::{Path, PathBuf};

/// Paths produced by the alpine.rhai recipe after execution.
#[derive(Debug, Clone)]
pub struct AlpinePaths {
    /// Path to the downloaded Alpine ISO.
    pub iso: PathBuf,
    /// Path to the extracted rootfs.
    pub rootfs: PathBuf,
}

impl AlpinePaths {
    /// Check if all paths exist.
    pub fn exists(&self) -> bool {
        self.iso.exists() && self.rootfs.exists()
    }
}

/// Run the alpine.rhai recipe and return the output paths.
///
/// This is the entry point for acornos to use recipe for Alpine dependency.
/// The recipe returns a ctx with paths, so we don't need to hardcode them.
///
/// # Arguments
/// * `base_dir` - acornos crate root (e.g., `/path/to/AcornOS`)
///
/// # Returns
/// The paths to the Alpine artifacts (ISO and rootfs).
pub fn alpine(base_dir: &Path) -> Result<AlpinePaths> {
    let monorepo_dir = base_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base_dir.to_path_buf());

    let downloads_dir = base_dir.join("downloads");
    let recipe_path = base_dir.join("deps/alpine.rhai");

    ensure_exists(&recipe_path, "Alpine recipe").map_err(|_| {
        anyhow::anyhow!(
            "Alpine recipe not found at: {}\n\
             Expected alpine.rhai in AcornOS/deps/",
            recipe_path.display()
        )
    })?;

    // Find and run recipe, parse JSON output
    let recipe_bin = find_recipe(&monorepo_dir)?;
    let ctx = run_recipe_json(&recipe_bin.path, &recipe_path, &downloads_dir)?;

    // Extract paths from ctx (recipe sets these)
    let iso = ctx["iso_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("alpine-extended-latest-x86_64.iso"));

    let rootfs = ctx["rootfs_path"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| downloads_dir.join("rootfs"));

    let paths = AlpinePaths { iso, rootfs };

    if !paths.exists() {
        bail!(
            "Recipe completed but expected paths are missing:\n\
             - ISO:    {} ({})\n\
             - rootfs: {} ({})",
            paths.iso.display(),
            if paths.iso.exists() { "OK" } else { "MISSING" },
            paths.rootfs.display(),
            if paths.rootfs.exists() {
                "OK"
            } else {
                "MISSING"
            },
        );
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// Verify that the extracted rootfs contains the FHS directory structure
    /// and all required packages (musl, busybox, apk-tools at minimum).
    #[test]
    fn test_extracted_rootfs_structure() {
        // This test assumes alpine.rhai has been run already
        let acorn_dir = Path::new("/home/vince/Projects/LevitateOS/AcornOS");
        let rootfs = acorn_dir.join("downloads/rootfs");

        // Skip if rootfs doesn't exist (test environment may not have run alpine.rhai)
        if !rootfs.exists() {
            eprintln!("Skipping rootfs structure test (rootfs not extracted yet)");
            return;
        }

        // === FHS Directory Structure ===
        // Required: /bin, /etc, /lib, /usr, /var, /tmp, /proc, /sys, /dev, /run, /home, /root
        let required_dirs = vec![
            "bin", "etc", "lib", "usr", "var", "tmp", "proc", "sys", "dev", "run", "home", "root",
        ];

        for dir in required_dirs {
            let path = rootfs.join(dir);
            assert!(
                path.is_dir(),
                "Missing required FHS directory: {}/{}",
                rootfs.display(),
                dir
            );
        }

        // === musl C library ===
        let musl_ld = rootfs.join("lib/ld-musl-x86_64.so.1");
        assert!(
            musl_ld.is_file(),
            "Missing musl libc loader: {}",
            musl_ld.display()
        );

        let musl_link = rootfs.join("lib/libc.musl-x86_64.so.1");
        assert!(
            musl_link.is_symlink(),
            "Missing musl libc symlink: {}",
            musl_link.display()
        );

        // === busybox ===
        let busybox = rootfs.join("bin/busybox");
        assert!(busybox.is_file(), "Missing busybox: {}", busybox.display());

        // Verify busybox has symlinks to shell and other core utilities
        // Note: sh and ash must be busybox, but other utilities may be from coreutils
        let shell_commands = vec!["sh", "ash"];
        for cmd in shell_commands {
            let link = rootfs.join(format!("bin/{}", cmd));
            assert!(
                link.is_symlink(),
                "Missing shell command symlink: bin/{}",
                cmd
            );
            let target = fs::read_link(&link).expect("Failed to read symlink");
            assert_eq!(
                target.to_string_lossy(),
                "/bin/busybox",
                "Shell command {} should point to busybox",
                cmd
            );
        }

        // Verify busybox is executable
        let metadata = fs::metadata(&busybox).expect("Failed to read busybox metadata");
        assert!(metadata.is_file(), "busybox is not a file");
        #[cfg(unix)]
        {
            assert!(
                metadata.permissions().mode() & 0o111 != 0,
                "busybox is not executable"
            );
        }

        // === apk-tools ===
        let apk = rootfs.join("sbin/apk");
        assert!(apk.is_file(), "Missing apk-tools: {}", apk.display());

        // === APK repositories configured ===
        let repos_file = rootfs.join("etc/apk/repositories");
        assert!(
            repos_file.is_file(),
            "Missing APK repositories: {}",
            repos_file.display()
        );

        let repos_content =
            fs::read_to_string(&repos_file).expect("Failed to read APK repositories");
        assert!(
            repos_content.contains("main"),
            "APK repositories missing Alpine main"
        );
        assert!(
            repos_content.contains("community"),
            "APK repositories missing Alpine community"
        );
    }
}
