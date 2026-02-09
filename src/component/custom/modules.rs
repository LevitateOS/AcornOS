//! Kernel module operations.
//!
//! Delegates to shared distro-builder implementation with AcornOS-specific hints.

use super::super::context::BuildContext;
use anyhow::Result;

pub fn copy_modules(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::modules::copy_modules(ctx, "acornos build kernel")
}

pub fn run_depmod(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::modules::run_depmod(ctx)
}
