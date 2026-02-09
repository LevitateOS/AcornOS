//! Linux kernel via recipe — AcornOS wrapper.

pub use distro_builder::recipe::linux::{has_linux_source, LinuxPaths};

use anyhow::Result;
use std::path::Path;

/// Run the linux.rhai recipe and return the output paths.
pub fn linux(base_dir: &Path) -> Result<LinuxPaths> {
    distro_builder::recipe::linux::linux(base_dir, "AcornOS")
}
