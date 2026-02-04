//! Alpine signing key setup and verification.
//!
//! This module handles copying Alpine signing keys from distro-spec into the rootfs
//! at `/etc/apk/keys/` so that APK can verify package signatures during installation.
//!
//! Keys are embedded in distro_spec::acorn::packages::ALPINE_KEYS and must be
//! installed before APK is used to install packages.

use anyhow::{bail, Context, Result};
use distro_spec::acorn::packages::ALPINE_KEYS;
use std::fs;
use std::path::Path;

/// Install Alpine signing keys into the rootfs.
///
/// # Arguments
/// * `rootfs_path` - Path to the rootfs where keys should be installed
///
/// # Returns
/// Result indicating success or failure
pub fn install_keys(rootfs_path: &Path) -> Result<()> {
    let keys_dir = rootfs_path.join("etc/apk/keys");

    // Ensure the keys directory exists
    fs::create_dir_all(&keys_dir)
        .with_context(|| format!("Failed to create keys directory: {}", keys_dir.display()))?;

    // Install each key
    for (filename, content) in ALPINE_KEYS {
        let key_path = keys_dir.join(filename);

        fs::write(&key_path, content)
            .with_context(|| format!("Failed to write key file: {}", key_path.display()))?;

        // Verify the key was written correctly
        if !key_path.exists() {
            bail!("Key file was not written: {}", key_path.display());
        }

        // Verify key content is valid (contains PEM header)
        let written_content = fs::read_to_string(&key_path)
            .with_context(|| format!("Failed to read back key file: {}", key_path.display()))?;

        if !written_content.contains("BEGIN PUBLIC KEY") {
            bail!(
                "Key file {} does not contain valid PEM format",
                key_path.display()
            );
        }
    }

    println!(
        "  Alpine signing keys installed ({} keys)",
        ALPINE_KEYS.len()
    );

    Ok(())
}

/// Verify that Alpine signing keys are properly installed in the rootfs.
///
/// # Arguments
/// * `rootfs_path` - Path to the rootfs to verify
///
/// # Returns
/// Result indicating verification success or failure
pub fn verify_keys(rootfs_path: &Path) -> Result<()> {
    let keys_dir = rootfs_path.join("etc/apk/keys");

    if !keys_dir.exists() {
        bail!(
            "Alpine keys directory does not exist: {}",
            keys_dir.display()
        );
    }

    // Verify each expected key file exists
    for (filename, _) in ALPINE_KEYS {
        let key_path = keys_dir.join(filename);

        if !key_path.exists() {
            bail!("Alpine signing key missing: {}", key_path.display());
        }

        // Verify it's readable and contains valid PEM format
        let content = fs::read_to_string(&key_path)
            .with_context(|| format!("Failed to read key file: {}", key_path.display()))?;

        if !content.contains("BEGIN PUBLIC KEY") {
            bail!(
                "Key file {} does not contain valid PEM format",
                key_path.display()
            );
        }
    }

    println!(
        "  Alpine signing keys verified ({} keys)",
        ALPINE_KEYS.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_install_keys() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

        // Install keys
        install_keys(rootfs)?;

        // Verify all keys are present
        let keys_dir = rootfs.join("etc/apk/keys");
        assert!(keys_dir.exists(), "Keys directory should be created");

        for (filename, _) in ALPINE_KEYS {
            let key_path = keys_dir.join(filename);
            assert!(key_path.exists(), "Key file {} should exist", filename);

            let content = fs::read_to_string(&key_path)?;
            assert!(
                content.contains("BEGIN PUBLIC KEY"),
                "Key {} should contain PEM format",
                filename
            );
        }

        Ok(())
    }

    #[test]
    fn test_verify_keys() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

        // Install keys first
        install_keys(rootfs)?;

        // Verify should succeed
        verify_keys(rootfs)?;

        Ok(())
    }

    #[test]
    fn test_verify_keys_missing_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

        // Don't install keys - verification should fail
        let result = verify_keys(rootfs);
        assert!(
            result.is_err(),
            "Verification should fail when keys dir missing"
        );

        Ok(())
    }

    #[test]
    fn test_verify_keys_missing_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

        // Create keys directory but don't install keys
        fs::create_dir_all(rootfs.join("etc/apk/keys"))?;

        // Verification should fail
        let result = verify_keys(rootfs);
        assert!(
            result.is_err(),
            "Verification should fail when key files missing"
        );

        Ok(())
    }
}
