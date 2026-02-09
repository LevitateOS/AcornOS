//! SSH configuration and host key generation.
//!
//! Delegates to shared distro-builder implementation with AcornOS-specific comment.

use crate::component::BuildContext;
use anyhow::Result;

pub fn setup_ssh(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::ssh::setup_ssh(ctx, "root@acornos")
}
