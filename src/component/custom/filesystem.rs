//! Filesystem custom operations.
//!
//! Delegates to shared distro-builder implementation.

use super::super::context::BuildContext;
use anyhow::Result;

pub fn create_fhs_symlinks(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::filesystem::create_fhs_symlinks(ctx)
}

pub fn setup_device_manager(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::filesystem::setup_device_manager(ctx)
}

pub fn copy_all_libraries(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::filesystem::copy_all_libraries(ctx)
}
