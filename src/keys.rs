//! Alpine signing key setup and verification for AcornOS.
//!
//! Delegates to shared distro-builder key management with AcornOS-specific keys.

use anyhow::Result;
use distro_spec::acorn::packages::ALPINE_KEYS;
use std::path::Path;

/// Install Alpine signing keys into the rootfs.
pub fn install_keys(rootfs_path: &Path) -> Result<()> {
    distro_builder::alpine::keys::install_keys(rootfs_path, ALPINE_KEYS)
}

/// Verify that Alpine signing keys are properly installed in the rootfs.
pub fn verify_keys(rootfs_path: &Path) -> Result<()> {
    distro_builder::alpine::keys::verify_keys(rootfs_path, ALPINE_KEYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    use distro_spec::acorn::packages::ALPINE_KEYS;

    #[test]
    fn test_install_keys() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

        install_keys(rootfs)?;

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

        install_keys(rootfs)?;
        verify_keys(rootfs)?;

        Ok(())
    }

    #[test]
    fn test_verify_keys_missing_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let rootfs = temp_dir.path();

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

        fs::create_dir_all(rootfs.join("etc/apk/keys"))?;

        let result = verify_keys(rootfs);
        assert!(
            result.is_err(),
            "Verification should fail when key files missing"
        );

        Ok(())
    }
}
