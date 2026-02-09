//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module handles a specific domain of custom operations.

mod branding;
mod live;

use anyhow::Result;

use super::BuildContext;
use super::CustomOp;

/// Execute a custom operation.
pub fn execute(ctx: &BuildContext, op: CustomOp) -> Result<()> {
    match op {
        // Filesystem operations
        CustomOp::CreateFhsSymlinks => distro_builder::alpine::filesystem::create_fhs_symlinks(ctx),

        // Branding
        CustomOp::CreateOsRelease => branding::create_os_release(ctx),
        CustomOp::CreateBranding => branding::create_branding(ctx),
        CustomOp::CreateEtcFiles => branding::create_etc_files(ctx),
        CustomOp::CreateSecurityConfig => branding::create_security_config(ctx),

        // Busybox
        CustomOp::CreateBusyboxApplets => {
            distro_builder::alpine::busybox::create_applet_symlinks(ctx)
        }

        // Device manager
        CustomOp::SetupDeviceManager => {
            distro_builder::alpine::filesystem::setup_device_manager(ctx)
        }

        // Kernel modules
        CustomOp::CopyModules => {
            distro_builder::alpine::modules::copy_modules(ctx, "acornos build kernel")
        }
        CustomOp::RunDepmod => distro_builder::alpine::modules::run_depmod(ctx),

        // Firmware
        CustomOp::CopyWifiFirmware => distro_builder::alpine::firmware::copy_wifi_firmware(ctx),
        CustomOp::CopyAllFirmware => distro_builder::alpine::firmware::copy_all_firmware(ctx),

        // Timezone
        CustomOp::CopyTimezoneData => branding::copy_timezone_data(ctx),

        // Live ISO
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CopyRecstrap => live::copy_recstrap(ctx),

        // Libraries
        CustomOp::CopyAllLibraries => distro_builder::alpine::filesystem::copy_all_libraries(ctx),

        // SSH
        CustomOp::SetupSsh => distro_builder::alpine::ssh::setup_ssh(ctx, "root@acornos"),
    }
}
