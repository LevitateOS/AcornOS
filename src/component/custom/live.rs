//! Live ISO custom operations.
//!
//! Creates welcome message, live overlay, and installer tools.

use anyhow::Result;
use std::fs;

use crate::component::BuildContext;
use distro_spec::acorn::{LIVE_ISSUE_MESSAGE, OS_NAME};

/// Create welcome message for live ISO.
pub fn create_welcome_message(ctx: &BuildContext) -> Result<()> {
    let staging = &ctx.staging;

    // Create /etc/issue.net
    fs::write(staging.join("etc/issue.net"), LIVE_ISSUE_MESSAGE)?;

    // Ensure profile.d directory exists
    let profile_d_path = staging.join("etc/profile.d");
    fs::create_dir_all(&profile_d_path)?;

    // Create a welcome script that runs on login
    let welcome_script = format!(
        r#"#!/bin/sh
# Welcome script for {} Live

echo ""
echo "Welcome to {} Live!"
echo ""
echo "To install {} to disk:"
echo "  1. Partition your disk (fdisk, parted, or gdisk)"
echo "  2. Run: recstrap /dev/sdX"
echo ""
echo "For help: docs-tui (if available) or visit https://levitateos.org/acorn/docs"
echo ""
"#,
        OS_NAME, OS_NAME, OS_NAME
    );

    let welcome_path = staging.join("etc/profile.d/welcome.sh");
    fs::write(&welcome_path, welcome_script)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&welcome_path, fs::Permissions::from_mode(0o755))?;

    // Copy test instrumentation scripts from profile/live-overlay/etc/profile.d/
    let overlay_profile_d = ctx.base_dir.join("profile/live-overlay/etc/profile.d");
    if overlay_profile_d.exists() {
        for entry in fs::read_dir(&overlay_profile_d)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_name = entry.file_name();
                if let Some(name_str) = file_name.to_str() {
                    // Skip welcome.sh since we already created it above
                    if name_str != "welcome.sh" {
                        let src = path;
                        let dst = profile_d_path.join(&file_name);
                        fs::copy(&src, &dst)?;
                        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Create live overlay directory structure.
///
/// The live ISO uses an overlay filesystem:
/// - Lower layer: EROFS (read-only)
/// - Upper layer: tmpfs (read-write)
///
/// This creates the directories needed for the overlay.
pub fn create_live_overlay(ctx: &BuildContext) -> Result<()> {
    let staging = &ctx.staging;

    // The overlay directories are created at boot by initramfs,
    // but we need to ensure the mount points exist in the EROFS image.

    // /run is used for runtime data
    fs::create_dir_all(staging.join("run"))?;

    // Ensure /tmp exists with proper permissions
    let tmp = staging.join("tmp");
    fs::create_dir_all(&tmp)?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o1777))?;

    Ok(())
}

/// Copy recstrap installer tools.
///
/// recstrap is the AcornOS equivalent of pacstrap - it extracts
/// the EROFS image to a target disk for installation.
pub fn copy_recstrap(ctx: &BuildContext) -> Result<()> {
    let staging = &ctx.staging;

    // Check if we have a pre-built recstrap binary
    let recstrap_candidates = [
        ctx.base_dir
            .join("../tools/recstrap/target/release/recstrap"),
        ctx.base_dir.join("../target/release/recstrap"),
    ];

    let mut found_recstrap = false;
    for candidate in &recstrap_candidates {
        if candidate.exists() {
            let dst = staging.join("usr/bin/recstrap");
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(candidate, &dst)?;
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
            found_recstrap = true;
            println!("  Copied recstrap installer tool");
            break;
        }
    }

    if !found_recstrap {
        // Create a placeholder script that explains how to install
        let placeholder = r#"#!/bin/sh
# recstrap - AcornOS installer
#
# The recstrap binary was not found during build.
# To install AcornOS manually:
#
# 1. Partition your disk:
#    fdisk /dev/sdX
#    # Create: 512MB EFI partition (type EFI System)
#    # Create: Rest as Linux partition
#
# 2. Format partitions:
#    mkfs.fat -F32 /dev/sdX1
#    mkfs.ext4 /dev/sdX2
#
# 3. Mount and extract:
#    mount /dev/sdX2 /mnt
#    mkdir -p /mnt/boot/efi
#    mount /dev/sdX1 /mnt/boot/efi
#    # Mount the EROFS image and copy files
#    mkdir -p /tmp/erofs
#    mount -t erofs /media/cdrom/live/filesystem.erofs /tmp/erofs
#    cp -a /tmp/erofs/* /mnt/
#
# 4. Install bootloader:
#    arch-chroot /mnt
#    grub-install --target=x86_64-efi --efi-directory=/boot/efi
#    grub-mkconfig -o /boot/grub/grub.cfg
#
# 5. Set root password and reboot

echo "recstrap binary not available - see script for manual install instructions"
echo "View this script: cat /usr/bin/recstrap"
exit 1
"#;
        let dst = staging.join("usr/bin/recstrap");
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dst, placeholder)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
        println!("  Created recstrap placeholder (binary not found)");
    }

    // Also copy recfstab and recchroot if available
    for tool in &["recfstab", "recchroot"] {
        let candidates = [
            ctx.base_dir
                .join(format!("../tools/{}/target/release/{}", tool, tool)),
            ctx.base_dir.join(format!("../target/release/{}", tool)),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                let dst = staging.join("usr/bin").join(tool);
                fs::copy(candidate, &dst)?;
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dst, fs::Permissions::from_mode(0o755))?;
                println!("  Copied {} tool", tool);
                break;
            }
        }
    }

    Ok(())
}
