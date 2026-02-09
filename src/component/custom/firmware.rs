//! Firmware custom operations.
//!
//! Delegates to shared distro-builder implementation.

use super::super::context::BuildContext;
use anyhow::Result;

pub fn copy_wifi_firmware(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::firmware::copy_wifi_firmware(ctx)
}

pub fn copy_all_firmware(ctx: &BuildContext) -> Result<()> {
    distro_builder::alpine::firmware::copy_all_firmware(ctx)
}
