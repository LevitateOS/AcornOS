//! AcornOS distribution configuration.
//!
//! Implements [`DistroConfig`] for AcornOS, providing all the
//! distro-specific constants needed by the build infrastructure.
//!
//! # Example
//!
//! ```rust
//! use acornos::config::AcornConfig;
//! use distro_builder::DistroConfig;
//!
//! let config = AcornConfig;
//! assert_eq!(config.os_name(), "AcornOS");
//! assert_eq!(config.default_shell(), "/bin/ash");
//! ```

use distro_contract::{DistroConfig, InitSystem, KernelInstallConfig};

/// AcornOS distribution configuration.
///
/// This struct implements [`DistroConfig`] by delegating to
/// constants defined in [`distro_spec::acorn`].
pub struct AcornConfig;

impl KernelInstallConfig for AcornConfig {
    fn module_install_path(&self) -> &str {
        distro_spec::acorn::MODULE_INSTALL_PATH
    }

    fn kernel_filename(&self) -> &str {
        distro_spec::acorn::KERNEL_FILENAME
    }
}

impl DistroConfig for AcornConfig {
    fn os_name(&self) -> &str {
        distro_spec::acorn::OS_NAME
    }

    fn os_id(&self) -> &str {
        distro_spec::acorn::OS_ID
    }

    fn iso_label(&self) -> &str {
        distro_spec::acorn::ISO_LABEL
    }

    fn boot_modules(&self) -> &[&str] {
        distro_spec::acorn::BOOT_MODULES
    }

    fn default_shell(&self) -> &str {
        distro_spec::acorn::DEFAULT_SHELL
    }

    fn init_system(&self) -> InitSystem {
        InitSystem::OpenRC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acorn_config() {
        let config = AcornConfig;

        assert_eq!(config.os_name(), "AcornOS");
        assert_eq!(config.os_id(), "acornos");
        assert_eq!(config.iso_label(), "ACORNOS");
        assert_eq!(config.default_shell(), "/bin/ash");
        assert_eq!(config.init_system(), InitSystem::OpenRC);
    }

    #[test]
    fn test_boot_modules_not_empty() {
        let config = AcornConfig;
        assert!(!config.boot_modules().is_empty());
    }
}
