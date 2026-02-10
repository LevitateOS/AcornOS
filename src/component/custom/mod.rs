//! Custom operations that require imperative code.
//!
//! These operations have complex logic that doesn't fit the declarative pattern.
//! Each module handles a specific domain of custom operations.

mod branding;
mod live;

use anyhow::Result;

use distro_builder::LicenseTracker;
use distro_spec::shared::auth::ssh::SSHD_CONFIG_SETTINGS;
use distro_spec::shared::busybox::{COMMON_APPLETS, SBIN_APPLETS};
use distro_spec::shared::components::{FHS_SYMLINKS, VAR_SYMLINKS};
use distro_spec::shared::firmware::WIFI_FIRMWARE_DIRS;
use distro_spec::shared::modules::MODULE_METADATA_FILES;
use distro_spec::shared::paths::LIBRARY_DIRS;

use super::BuildContext;
use super::CustomOp;

/// Execute a custom operation.
///
/// Some operations copy content that requires license tracking. The tracker
/// is used to register packages for license compliance.
pub fn execute(ctx: &BuildContext, op: CustomOp, tracker: &LicenseTracker) -> Result<()> {
    match op {
        // Filesystem operations (no content copying)
        CustomOp::CreateFhsSymlinks => {
            distro_builder::alpine::filesystem::create_fhs_symlinks(ctx, FHS_SYMLINKS, VAR_SYMLINKS)
        }

        // Branding (config files only)
        CustomOp::CreateEtcFiles => branding::create_etc_files(ctx),
        CustomOp::CreateSecurityConfig => branding::create_security_config(ctx),

        // Busybox (symlinks only, busybox itself tracked via Op::Bin)
        CustomOp::CreateBusyboxApplets => distro_builder::alpine::busybox::create_applet_symlinks(
            ctx,
            SBIN_APPLETS,
            COMMON_APPLETS,
        ),

        // Device manager (config only)
        CustomOp::SetupDeviceManager => {
            distro_builder::alpine::filesystem::setup_device_manager(ctx)
        }

        // Kernel modules - register kernel package
        CustomOp::CopyModules => {
            tracker.register_package("linux-lts");
            distro_builder::alpine::modules::copy_modules(
                ctx,
                "acornos build kernel",
                MODULE_METADATA_FILES,
            )
        }

        // Firmware - register linux-firmware package
        CustomOp::CopyWifiFirmware => {
            tracker.register_package("linux-firmware");
            distro_builder::alpine::firmware::copy_firmware_dirs(ctx, WIFI_FIRMWARE_DIRS)
        }

        // Timezone - register tzdata package
        CustomOp::CopyTimezoneData => {
            tracker.register_package("tzdata");
            branding::copy_timezone_data(ctx)
        }

        // Live ISO (generated content, no third-party packages)
        CustomOp::CreateWelcomeMessage => live::create_welcome_message(ctx),
        CustomOp::CreateLiveOverlay => live::create_live_overlay(ctx),
        CustomOp::CopyRecstrap => live::copy_recstrap(ctx),

        // Libraries - register musl (the libc providing most .so files)
        CustomOp::CopyAllLibraries => {
            tracker.register_package("musl");
            distro_builder::alpine::filesystem::copy_all_libraries(ctx, LIBRARY_DIRS)
        }

        // SSH - register openssh package
        CustomOp::SetupSsh => {
            tracker.register_package("openssh");
            distro_builder::alpine::ssh::setup_ssh(ctx, "root@acornos", SSHD_CONFIG_SETTINGS)
        }
    }
}
