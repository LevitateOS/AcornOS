//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module handles a specific domain of custom operations.

mod branding;
mod busybox;
mod filesystem;
mod firmware;
mod live;

use anyhow::Result;

use super::context::BuildContext;
use super::CustomOp;

/// Execute a custom operation.
pub fn execute(ctx: &BuildContext, op: CustomOp) -> Result<()> {
    match op {
        // Filesystem operations
        CustomOp::CreateFhsSymlinks => filesystem::create_fhs_symlinks(ctx),

        // Branding
        CustomOp::CreateOsRelease => branding::create_os_release(ctx),
        CustomOp::CreateBranding => branding::create_branding(ctx),
        CustomOp::CreateEtcFiles => branding::create_etc_files(ctx),

        // Busybox
        CustomOp::CreateBusyboxApplets => busybox::create_applet_symlinks(ctx),

        // Device manager
        CustomOp::SetupDeviceManager => filesystem::setup_device_manager(ctx),

        // Firmware
        CustomOp::CopyWifiFirmware => firmware::copy_wifi_firmware(ctx),
        CustomOp::CopyAllFirmware => firmware::copy_all_firmware(ctx),

        // Timezone
        CustomOp::CopyTimezoneData => branding::copy_timezone_data(ctx),

        // Live ISO
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CopyRecstrap => live::copy_recstrap(ctx),
    }
}
