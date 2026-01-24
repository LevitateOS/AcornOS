//! AcornOS kernel and build configuration.
//!
//! # Status: SKELETON
//!
//! Placeholder for AcornOS-specific kernel configuration.

use distro_builder::build::context::{DistroConfig, InitSystem};

/// AcornOS distribution configuration.
pub struct AcornConfig;

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

    fn squashfs_compression(&self) -> &str {
        distro_spec::acorn::SQUASHFS_COMPRESSION
    }

    fn squashfs_block_size(&self) -> &str {
        distro_spec::acorn::SQUASHFS_BLOCK_SIZE
    }
}
