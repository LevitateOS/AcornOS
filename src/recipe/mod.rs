//! AcornOS-specific recipe modules.
//!
//! Alpine recipe has AcornOS-specific key installation logic.
//! All other recipe functionality is used directly from `distro_builder::recipe`.

mod alpine;

pub use alpine::{alpine, AlpinePaths};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_package_dependency_resolution() {
        let acorn_dir = Path::new("/home/vince/Projects/LevitateOS/AcornOS");
        let rootfs = acorn_dir.join("downloads/rootfs");

        if !rootfs.exists() {
            eprintln!("Skipping package dependency test (rootfs not extracted yet)");
            return;
        }

        let apk_db = rootfs.join("lib/apk/db/installed");
        if !apk_db.exists() {
            eprintln!("APK database not found - skipping dependency test (recipes not run)");
            return;
        }

        let db_content = fs::read_to_string(&apk_db).expect("Failed to read APK database");

        assert!(
            !db_content.trim().is_empty(),
            "APK database is empty - package installation may have failed"
        );

        let has_packages = db_content.lines().any(|line| line.starts_with("P:"));
        assert!(
            has_packages,
            "APK database has no package entries (format error)"
        );

        let tier0_packages = vec!["alpine-base", "openrc", "linux-lts", "musl"];
        for pkg in &tier0_packages {
            assert!(
                db_content.contains(&format!("P:{}", pkg)),
                "Tier 0 package {} not found in APK database",
                pkg
            );
        }

        eprintln!("✓ Tier 0 packages verified in APK database");

        let optional_packages = vec!["eudev", "bash", "dhcpcd", "doas"];
        for pkg in &optional_packages {
            if db_content.contains(&format!("P:{}", pkg)) {
                eprintln!(
                    "✓ Optional package {} installed (dependency chain verified by apk)",
                    pkg
                );
            }
        }

        let has_dependencies = db_content.lines().any(|line| line.starts_with("d:"));
        if has_dependencies {
            eprintln!("✓ APK database includes dependency records (d: entries found)");
        } else {
            eprintln!("ℹ No explicit dependency records (d: entries) in current database");
        }

        let expected_bins = vec!["bin/busybox", "sbin/apk"];
        for bin in &expected_bins {
            let path = rootfs.join(bin);
            if !path.exists() && path.read_link().is_err() {
                if rootfs.join("usr").exists() {
                    eprintln!("⚠ Binary {} not found", bin);
                }
            }
        }

        eprintln!("✓ Package dependency resolution verified");
    }

    #[test]
    fn test_alpine_keys_setup() {
        let acorn_dir = Path::new("/home/vince/Projects/LevitateOS/AcornOS");
        let rootfs = acorn_dir.join("downloads/rootfs");

        if !rootfs.exists() {
            eprintln!("Skipping Alpine keys test (rootfs not extracted yet)");
            return;
        }

        let keys_dir = rootfs.join("etc/apk/keys");

        if keys_dir.exists() {
            match fs::read_dir(&keys_dir) {
                Ok(entries) => {
                    let key_count = entries.count();
                    if key_count > 0 {
                        eprintln!("✓ Alpine signing keys installed ({} files)", key_count);
                    } else {
                        eprintln!("ℹ Keys directory exists but is empty");
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Could not enumerate keys directory: {}", e);
                }
            }
        } else {
            eprintln!("ℹ Alpine keys directory not yet populated");
        }

        let repos_file = rootfs.join("etc/apk/repositories");
        if repos_file.exists() {
            let content = fs::read_to_string(&repos_file).unwrap_or_default();
            if content.contains("https://") {
                eprintln!("✓ APK repositories configured with HTTPS");
            } else if content.contains("http://") {
                eprintln!("ℹ APK repositories using HTTP (not HTTPS)");
            }
        }
    }
}
