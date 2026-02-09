//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module handles a specific domain of custom operations.

mod branding;
mod live;

use anyhow::Result;

use distro_spec::shared::auth::ssh::SSHD_CONFIG_SETTINGS;
use distro_spec::shared::busybox::{COMMON_APPLETS, SBIN_APPLETS};
use distro_spec::shared::components::{FHS_SYMLINKS, VAR_SYMLINKS};
use distro_spec::shared::firmware::WIFI_FIRMWARE_DIRS;
use distro_spec::shared::modules::MODULE_METADATA_FILES;
use distro_spec::shared::paths::LIBRARY_DIRS;

use super::BuildContext;
use super::CustomOp;

/// Execute a custom operation.
pub fn execute(ctx: &BuildContext, op: CustomOp) -> Result<()> {
    match op {
        // Filesystem operations
        CustomOp::CreateFhsSymlinks => {
            distro_builder::alpine::filesystem::create_fhs_symlinks(ctx, FHS_SYMLINKS, VAR_SYMLINKS)
        }

        // Branding
        CustomOp::CreateEtcFiles => branding::create_etc_files(ctx),
        CustomOp::CreateSecurityConfig => branding::create_security_config(ctx),

        // Busybox
        CustomOp::CreateBusyboxApplets => distro_builder::alpine::busybox::create_applet_symlinks(
            ctx,
            SBIN_APPLETS,
            COMMON_APPLETS,
        ),

        // Device manager
        CustomOp::SetupDeviceManager => {
            distro_builder::alpine::filesystem::setup_device_manager(ctx)
        }

        // Kernel modules
        CustomOp::CopyModules => distro_builder::alpine::modules::copy_modules(
            ctx,
            "acornos build kernel",
            MODULE_METADATA_FILES,
        ),

        // Firmware
        CustomOp::CopyWifiFirmware => {
            distro_builder::alpine::firmware::copy_firmware_dirs(ctx, WIFI_FIRMWARE_DIRS)
        }

        // Timezone
        CustomOp::CopyTimezoneData => branding::copy_timezone_data(ctx),

        // Live ISO
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CopyRecstrap => live::copy_recstrap(ctx),

        // Libraries
        CustomOp::CopyAllLibraries => {
            distro_builder::alpine::filesystem::copy_all_libraries(ctx, LIBRARY_DIRS)
        }

        // SSH
        CustomOp::SetupSsh => {
            distro_builder::alpine::ssh::setup_ssh(ctx, "root@acornos", SSHD_CONFIG_SETTINGS)
        }
    }
}
