//! Busybox custom operations.
//!
//! Delegates to shared distro-builder implementation.

use super::super::context::BuildContext;
use anyhow::Result;

pub fn create_applet_symlinks(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::busybox::create_applet_symlinks(ctx)
}
