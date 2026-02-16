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

        // Stage test scripts (no package tracking - local scripts)
        CustomOp::InstallStageTests => install_stage_tests(ctx),
    }
}

/// Install stage test scripts to the ISO.
///
/// These scripts are used for both automated testing and manual verification.
/// They validate each stage stage and provide detailed feedback.
fn install_stage_tests(ctx: &BuildContext) -> Result<()> {
    use std::fs;

    // Source: monorepo testing/install-tests/test-scripts/
    let monorepo_root = ctx
        .base_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine monorepo root"))?;
    let test_scripts_src = monorepo_root.join("testing/install-tests/test-scripts");

    if !test_scripts_src.exists() {
        anyhow::bail!(
            "Test scripts not found at: {}\n\
             Expected stage test scripts in testing/install-tests/test-scripts/",
            test_scripts_src.display()
        );
    }

    // Destination: /usr/local/bin/ for scripts, /usr/local/lib/stage-tests/ for libraries
    let bin_dst = ctx.staging.join("usr/local/bin");
    let lib_dst = ctx.staging.join("usr/local/lib/stage-tests");

    fs::create_dir_all(&bin_dst)?;
    fs::create_dir_all(&lib_dst)?;

    // Copy all .sh scripts to /usr/local/bin/
    let mut script_count = 0;
    for entry in fs::read_dir(&test_scripts_src)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|ext| ext == "sh") {
            let filename = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
            let dst = bin_dst.join(filename);

            fs::copy(&path, &dst)?;

            // Make executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dst)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dst, perms)?;
            }

            script_count += 1;
        }
    }

    // Copy lib/ directory to /usr/local/lib/stage-tests/
    let lib_src = test_scripts_src.join("lib");
    if lib_src.exists() {
        for entry in fs::read_dir(&lib_src)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let filename = path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                let dst = lib_dst.join(filename);

                fs::copy(&path, &dst)?;

                // Make library files executable (they may be sourced)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&dst, perms)?;
                }
            }
        }
    }

    println!(
        "  Installed {} stage test scripts to /usr/local/bin/",
        script_count
    );
    println!("  Installed stage test libraries to /usr/local/lib/stage-tests/");

    Ok(())
}
