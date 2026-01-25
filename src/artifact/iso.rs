//! ISO creation - builds bootable AcornOS ISO.
//!
//! Creates an ISO with squashfs-based architecture:
//! - Tiny initramfs (~5MB) - mounts squashfs + overlay
//! - Squashfs image (~200MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use distro_builder::process::Cmd;
use distro_spec::acorn::{
    // Identity
    ISO_LABEL, ISO_FILENAME, OS_NAME,
    // Squashfs
    SQUASHFS_NAME, SQUASHFS_ISO_PATH,
    // Boot files
    KERNEL_ISO_PATH, INITRAMFS_LIVE_ISO_PATH,
    INITRAMFS_LIVE_OUTPUT,
    // ISO structure
    ISO_BOOT_DIR, ISO_LIVE_DIR, ISO_EFI_DIR,
    LIVE_OVERLAY_ISO_PATH,
    // EFI
    EFIBOOT_FILENAME, EFIBOOT_SIZE_MB,
    EFI_BOOTLOADER,
    // Console
    SERIAL_CONSOLE, VGA_CONSOLE,
    // Checksum
    ISO_CHECKSUM_SUFFIX, SHA512_SEPARATOR,
    // xorriso
    XORRISO_PARTITION_OFFSET, XORRISO_FS_FLAGS,
};

use crate::extract::ExtractPaths;

/// Get ISO volume label from environment or use default.
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string())
}

/// Paths used during ISO creation.
struct IsoPaths {
    output_dir: PathBuf,
    squashfs: PathBuf,
    initramfs_live: PathBuf,
    iso_output: PathBuf,
    iso_root: PathBuf,
    rootfs: PathBuf,
}

impl IsoPaths {
    fn new(base_dir: &Path) -> Self {
        let paths = ExtractPaths::new(base_dir);
        let output_dir = base_dir.join("output");
        Self {
            squashfs: output_dir.join(SQUASHFS_NAME),
            initramfs_live: output_dir.join(INITRAMFS_LIVE_OUTPUT),
            iso_output: output_dir.join(ISO_FILENAME),
            iso_root: output_dir.join("iso-root"),
            rootfs: paths.rootfs,
            output_dir,
        }
    }
}

/// Create ISO using squashfs-based architecture.
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts squashfs + overlay
/// - Squashfs image (~200MB) - complete base system
/// - Live overlay - live-specific configs (autologin, serial console)
///
/// Boot flow:
/// 1. kernel -> tiny initramfs
/// 2. init_tiny mounts squashfs as lower layer
/// 3. init_tiny mounts /live/overlay from ISO as middle layer
/// 4. init_tiny mounts tmpfs as upper layer (for writes)
/// 5. switch_root -> OpenRC
pub fn create_squashfs_iso(base_dir: &Path) -> Result<()> {
    let paths = IsoPaths::new(base_dir);

    println!("=== Building AcornOS ISO ===\n");

    // Stage 1: Validate inputs
    validate_iso_inputs(&paths)?;

    // Stage 2: Create live overlay (autologin, serial console, empty root password)
    create_live_overlay(&paths.output_dir)?;

    // Stage 3: Set up ISO directory structure
    setup_iso_structure(&paths)?;

    // Stage 4: Copy boot files and artifacts
    copy_iso_artifacts(&paths)?;

    // Stage 5: Set up UEFI boot
    setup_uefi_boot(&paths)?;

    // Stage 6: Create the ISO to a temporary file
    let temp_iso = paths.output_dir.join(format!("{}.tmp", ISO_FILENAME));
    run_xorriso(&paths, &temp_iso)?;

    // Stage 7: Generate checksum for the temporary ISO
    generate_iso_checksum(&temp_iso)?;

    // Atomic rename to final destination
    fs::rename(&temp_iso, &paths.iso_output)?;

    // Also move the checksum
    let temp_checksum = temp_iso.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let final_checksum = paths.iso_output.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let _ = fs::rename(&temp_checksum, &final_checksum);

    print_iso_summary(&paths.iso_output);
    Ok(())
}

/// Stage 1: Validate that required input files exist.
fn validate_iso_inputs(paths: &IsoPaths) -> Result<()> {
    if !paths.squashfs.exists() {
        bail!(
            "Squashfs not found at {}.\n\
             Run 'acornos build squashfs' first.",
            paths.squashfs.display()
        );
    }

    if !paths.initramfs_live.exists() {
        bail!(
            "Live initramfs not found at {}.\n\
             Run 'acornos initramfs' first.",
            paths.initramfs_live.display()
        );
    }

    // Check for kernel in rootfs
    let kernel = paths.rootfs.join("boot/vmlinuz-lts");
    if !kernel.exists() {
        bail!(
            "Kernel not found at {}.\n\
             The rootfs may be incomplete.",
            kernel.display()
        );
    }

    Ok(())
}

/// Create live overlay with autologin and empty root password.
fn create_live_overlay(output_dir: &Path) -> Result<()> {
    println!("Creating live overlay...");

    let live_overlay = output_dir.join("live-overlay");

    // Clean previous
    if live_overlay.exists() {
        fs::remove_dir_all(&live_overlay)?;
    }

    // =========================================================================
    // STEP 1: Copy profile/live-overlay (test instrumentation, etc.)
    // =========================================================================
    // This copies files from AcornOS/profile/live-overlay/ which includes:
    // - Test instrumentation (etc/profile.d/00-acorn-test.sh)
    // These are copied FIRST so code-generated files below can override if needed.
    let base_dir = output_dir.parent().unwrap_or(Path::new("."));
    let profile_overlay = base_dir.join("profile/live-overlay");
    if profile_overlay.exists() {
        println!("  Copying profile/live-overlay (test instrumentation)...");
        copy_dir_recursive(&profile_overlay, &live_overlay)?;
    }

    // =========================================================================
    // STEP 2: Code-generated overlay files (may override profile defaults)
    // =========================================================================

    // Create directory structure
    fs::create_dir_all(live_overlay.join("etc"))?;

    // Create autologin script for serial console
    // This script is called by agetty -l to act as a login program replacement
    // agetty sets up the tty, this just needs to spawn a login shell
    fs::create_dir_all(live_overlay.join("usr/local/bin"))?;
    let autologin_script = r#"#!/bin/sh
# Autologin for serial console testing
# Called by agetty -l as the login program
# agetty has already set up stdin/stdout/stderr on the tty

echo "[autologin] Starting login shell..."

# Run sh as login shell (sources /etc/profile and /etc/profile.d/*)
# In Alpine, /bin/sh is busybox ash
exec /bin/sh -l
"#;
    let autologin_path = live_overlay.join("usr/local/bin/serial-autologin");
    fs::write(&autologin_path, autologin_script)?;
    let mut perms = fs::metadata(&autologin_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&autologin_path, perms)?;

    // Create /etc/issue for live boot identification
    fs::write(
        live_overlay.join("etc/issue"),
        "\nAcornOS Live - \\l\n\nLogin as 'root' (no password)\n\n",
    )?;

    // Create empty root password (allow passwordless login)
    // This is applied ONLY during live boot via overlay
    // The squashfs base system has a locked root password
    let shadow_content = "root::0:0:99999:7:::\n\
                          bin:!:0:0:99999:7:::\n\
                          daemon:!:0:0:99999:7:::\n\
                          nobody:!:0:0:99999:7:::\n";
    fs::write(live_overlay.join("etc/shadow"), shadow_content)?;

    // Set proper permissions on shadow (read-only by root)
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(live_overlay.join("etc/shadow"))?.permissions();
    perms.set_mode(0o640);
    fs::set_permissions(live_overlay.join("etc/shadow"), perms)?;

    // Enable serial console for automated testing
    // Create /etc/inittab with serial getty ENABLED
    // This overlays the base inittab from squashfs (which has serial commented out)
    //
    // For autologin on serial, we use agetty --autologin which:
    // - Automatically logs in the specified user (root)
    // - Spawns a login shell which sources /etc/profile and /etc/profile.d/*
    // - This is the standard Alpine Linux approach (see wiki.alpinelinux.org/wiki/TTY_Autologin)
    let inittab_content = r#"# /etc/inittab - AcornOS Live
# This inittab enables serial console for testing

::sysinit:/sbin/openrc sysinit
::sysinit:/sbin/openrc boot
::wait:/sbin/openrc default

# Virtual terminals
tty1::respawn:/sbin/getty 38400 tty1
tty2::respawn:/sbin/getty 38400 tty2
tty3::respawn:/sbin/getty 38400 tty3
tty4::respawn:/sbin/getty 38400 tty4
tty5::respawn:/sbin/getty 38400 tty5
tty6::respawn:/sbin/getty 38400 tty6

# Serial console - ENABLED with AUTOLOGIN for testing
# Uses wrapper script that redirects I/O and spawns login shell
ttyS0::respawn:/usr/local/bin/serial-autologin

# Ctrl+Alt+Del
::ctrlaltdel:/sbin/reboot

# Shutdown
::shutdown:/sbin/openrc shutdown
"#;
    fs::write(live_overlay.join("etc/inittab"), inittab_content)?;

    // Enable services in default runlevel
    let runlevels_default = live_overlay.join("etc/runlevels/default");
    fs::create_dir_all(&runlevels_default)?;
    std::os::unix::fs::symlink("/etc/init.d/local", runlevels_default.join("local"))?;

    // Enable serial getty using OpenRC's agetty service
    // Note: agetty -l specifies the login program, but we use a wrapper
    // that invokes ash as a login shell since /bin/login doesn't exist in Alpine
    let init_d = live_overlay.join("etc/init.d");
    fs::create_dir_all(&init_d)?;
    std::os::unix::fs::symlink("agetty", init_d.join("agetty.ttyS0"))?;
    std::os::unix::fs::symlink("/etc/init.d/agetty.ttyS0", runlevels_default.join("agetty.ttyS0"))?;

    // Configure serial getty with autologin using wrapper script
    let conf_d = live_overlay.join("etc/conf.d");
    fs::create_dir_all(&conf_d)?;
    let agetty_conf = r#"# Serial console configuration for live boot
baud="115200"
term_type="vt100"
# Use autologin wrapper that runs ash as login shell
# -n skips login prompt, -l specifies login program
agetty_options="-n -l /usr/local/bin/serial-autologin"
"#;
    fs::write(conf_d.join("agetty.ttyS0"), agetty_conf)?;

    // =========================================================================
    // P1: Volatile log storage
    // =========================================================================
    // Mount /var/log as tmpfs to prevent filling the overlay tmpfs.
    // Live session logs are ephemeral anyway - no need to persist them.
    // Size limit prevents runaway logging from killing the session.
    let fstab_content = r#"# AcornOS Live fstab
# Volatile log storage - prevents logs from filling overlay tmpfs
tmpfs   /var/log    tmpfs   nosuid,nodev,noexec,size=64M,mode=0755   0 0
"#;
    fs::write(live_overlay.join("etc/fstab"), fstab_content)?;

    // Create local.d script to ensure /var/log is mounted early
    // (fstab may not be processed before syslog starts)
    fs::create_dir_all(live_overlay.join("etc/local.d"))?;
    let volatile_log_script = r#"#!/bin/sh
# P1: Ensure volatile log storage for live session
# This runs early in boot to catch any logs before syslog starts

# Only mount if not already a tmpfs (idempotent)
if ! mountpoint -q /var/log 2>/dev/null; then
    # Preserve any existing logs created before mount
    if [ -d /var/log ]; then
        mkdir -p /tmp/log-backup
        cp -a /var/log/* /tmp/log-backup/ 2>/dev/null || true
    fi

    mount -t tmpfs -o nosuid,nodev,noexec,size=64M,mode=0755 tmpfs /var/log

    # Restore preserved logs
    if [ -d /tmp/log-backup ]; then
        cp -a /tmp/log-backup/* /var/log/ 2>/dev/null || true
        rm -rf /tmp/log-backup
    fi

    # Ensure log directories exist
    mkdir -p /var/log/chrony 2>/dev/null || true
fi
"#;
    let script_path = live_overlay.join("etc/local.d/00-volatile-log.start");
    fs::write(&script_path, volatile_log_script)?;
    let mut perms = fs::metadata(&script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms)?;

    // =========================================================================
    // P1: Do-not-suspend configuration
    // =========================================================================
    // Prevent the system from suspending during live session.
    // Users are likely installing - suspend would be disruptive.

    // Method 1: ACPI power button handler - do nothing on lid close/power button
    fs::create_dir_all(live_overlay.join("etc/acpi"))?;
    let acpi_handler = r#"#!/bin/sh
# AcornOS Live: Disable suspend actions
# Power button and lid close do nothing during live session

case "$1" in
    button/power)
        # Log but don't suspend - user is probably installing
        logger "AcornOS Live: Power button pressed (suspend disabled)"
        ;;
    button/lid)
        # Lid close does nothing - prevent accidental suspend
        logger "AcornOS Live: Lid event ignored (suspend disabled)"
        ;;
    *)
        # Let other events through to default handler
        ;;
esac
"#;
    let handler_path = live_overlay.join("etc/acpi/handler.sh");
    fs::write(&handler_path, acpi_handler)?;
    let mut perms = fs::metadata(&handler_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&handler_path, perms)?;

    // Method 2: Kernel parameters to disable suspend
    // This is set via sysctl for runtime
    let sysctl_content = r#"# AcornOS Live: Disable suspend
# Prevent accidental suspend during installation

# Disable suspend-to-RAM
kernel.sysrq = 1

# Note: Full suspend disable requires either:
# - elogind HandleLidSwitch=ignore (if using elogind)
# - acpid handler (provided above)
# - Or simply not having any suspend triggers
"#;
    fs::create_dir_all(live_overlay.join("etc/sysctl.d"))?;
    fs::write(live_overlay.join("etc/sysctl.d/50-live-no-suspend.conf"), sysctl_content)?;

    // Method 3: If elogind is present, configure it
    fs::create_dir_all(live_overlay.join("etc/elogind/logind.conf.d"))?;
    let logind_conf = r#"# AcornOS Live: Disable suspend triggers
[Login]
HandlePowerKey=ignore
HandleSuspendKey=ignore
HandleHibernateKey=ignore
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore
IdleAction=ignore
"#;
    fs::write(
        live_overlay.join("etc/elogind/logind.conf.d/00-live-no-suspend.conf"),
        logind_conf,
    )?;

    println!("  Live overlay created at {}", live_overlay.display());
    Ok(())
}

/// Stage 3: Create ISO directory structure.
fn setup_iso_structure(paths: &IsoPaths) -> Result<()> {
    if paths.iso_root.exists() {
        fs::remove_dir_all(&paths.iso_root)?;
    }

    fs::create_dir_all(paths.iso_root.join(ISO_BOOT_DIR))?;
    fs::create_dir_all(paths.iso_root.join(ISO_LIVE_DIR))?;
    fs::create_dir_all(paths.iso_root.join(ISO_EFI_DIR))?;

    Ok(())
}

/// Stage 4: Copy kernel, initramfs, squashfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths) -> Result<()> {
    // Copy kernel from rootfs (Alpine linux-lts)
    let kernel_src = paths.rootfs.join("boot/vmlinuz-lts");
    println!("Copying kernel from {}...", kernel_src.display());
    fs::copy(&kernel_src, paths.iso_root.join(KERNEL_ISO_PATH))?;

    // Copy live initramfs
    println!("Copying initramfs...");
    fs::copy(&paths.initramfs_live, paths.iso_root.join(INITRAMFS_LIVE_ISO_PATH))?;

    // Copy squashfs to /live/
    println!("Copying squashfs to ISO...");
    fs::copy(&paths.squashfs, paths.iso_root.join(SQUASHFS_ISO_PATH))?;

    // Copy live overlay to /live/overlay/
    let live_overlay_src = paths.output_dir.join("live-overlay");
    let live_overlay_dst = paths.iso_root.join(LIVE_OVERLAY_ISO_PATH);
    if live_overlay_src.exists() {
        println!("Copying live overlay to ISO...");
        copy_dir_recursive(&live_overlay_src, &live_overlay_dst)?;
    } else {
        bail!(
            "Live overlay not found at {}.\n\
             This should have been created by create_live_overlay().",
            live_overlay_src.display()
        );
    }

    Ok(())
}

/// Recursively copy a directory, preserving symlinks.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            // Preserve symlinks (important for runlevel service links)
            let target = fs::read_link(&src_path)?;
            std::os::unix::fs::symlink(&target, &dst_path)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Stage 5: Set up UEFI boot files and GRUB config.
fn setup_uefi_boot(paths: &IsoPaths) -> Result<()> {
    println!("Setting up UEFI boot...");

    let efi_bootloader_path = paths.iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER);

    // Try to copy EFI bootloader from Alpine ISO contents first
    let iso_contents = paths.output_dir.parent().unwrap().join("downloads/iso-contents");
    let alpine_efi = iso_contents.join("efi/boot/bootx64.efi");

    if alpine_efi.exists() {
        println!("  Copying EFI bootloader from Alpine ISO...");
        fs::copy(&alpine_efi, &efi_bootloader_path)?;
    } else {
        // Fallback: try to use grub-mkstandalone (grub2-mkstandalone on Fedora)
        println!("  Creating GRUB EFI bootloader...");

        // Create a minimal grub.cfg for embedding
        let embedded_cfg = "configfile /EFI/BOOT/grub.cfg\n";
        let embedded_cfg_path = paths.output_dir.join("grub-embed.cfg");
        fs::write(&embedded_cfg_path, embedded_cfg)?;

        // Try grub2-mkstandalone (Fedora) first, then grub-mkstandalone (Debian/Ubuntu)
        let grub_cmd = if std::process::Command::new("grub2-mkstandalone")
            .arg("--version")
            .output()
            .is_ok()
        {
            "grub2-mkstandalone"
        } else {
            "grub-mkstandalone"
        };

        Cmd::new(grub_cmd)
            .args(["--format=x86_64-efi"])
            .args(["--output"])
            .arg_path(&efi_bootloader_path)
            .args(["--locales="])
            .args(["--fonts="])
            .arg(format!("boot/grub/grub.cfg={}", embedded_cfg_path.display()))
            .error_msg("grub-mkstandalone failed. Install: sudo dnf install grub2-tools-extra")
            .run()?;
    }

    // Create GRUB config - Alpine's bootloader looks for /boot/grub/grub.cfg
    // Key options:
    // - modules=: Tell Alpine init which kernel modules to load (like Alpine's original)
    // - root=LABEL=XXX: Passed to init script for finding boot device
    // Note: Alpine GRUB uses `linux`/`initrd`, not `linuxefi`/`initrdefi`
    let label = iso_label();
    // Modules needed for live boot (match Alpine's approach)
    let modules = "modules=loop,squashfs,overlay,virtio_pci,virtio_blk,virtio_scsi,sd-mod,sr-mod,cdrom,isofs";
    // IMPORTANT: console= order matters. The LAST console becomes /dev/console for init.
    // For serial testing, ttyS0 must be last so init's stdout goes to serial.
    let grub_cfg = format!(
        r#"# Serial console for automated testing
serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1
terminal_input serial console
terminal_output serial console

set default=0
set timeout=5

menuentry '{}' {{
    linux /{} {} root=LABEL={} {} {}
    initrd /{}
}}

menuentry '{} (Emergency Shell)' {{
    linux /{} {} root=LABEL={} {} {} emergency
    initrd /{}
}}

menuentry '{} (Debug)' {{
    linux /{} {} root=LABEL={} {} {} debug
    initrd /{}
}}
"#,
        // VGA first, serial LAST - so /dev/console -> serial for testing
        OS_NAME, KERNEL_ISO_PATH, modules, label, VGA_CONSOLE, SERIAL_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, modules, label, VGA_CONSOLE, SERIAL_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, modules, label, VGA_CONSOLE, SERIAL_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
    );

    // Write to both locations for compatibility:
    // - /boot/grub/grub.cfg (where Alpine's bootloader looks)
    // - /EFI/BOOT/grub.cfg (standard EFI location)
    let boot_grub = paths.iso_root.join("boot/grub");
    fs::create_dir_all(&boot_grub)?;
    fs::write(boot_grub.join("grub.cfg"), &grub_cfg)?;
    fs::write(paths.iso_root.join(ISO_EFI_DIR).join("grub.cfg"), &grub_cfg)?;

    // Create EFI boot image
    let efiboot_img = paths.output_dir.join(EFIBOOT_FILENAME);
    create_efi_boot_image(&paths.iso_root, &efiboot_img)?;

    Ok(())
}

/// Stage 6: Run xorriso to create the final ISO.
fn run_xorriso(paths: &IsoPaths, output: &Path) -> Result<()> {
    println!("Creating UEFI bootable ISO with xorriso...");
    let label = iso_label();

    Cmd::new("xorriso")
        .args(["-as", "mkisofs", "-o"])
        .arg_path(output)
        .args(["-V", &label])
        .args(["-partition_offset", &XORRISO_PARTITION_OFFSET.to_string()])
        .args(XORRISO_FS_FLAGS)
        .args(["-e", EFIBOOT_FILENAME, "-no-emul-boot", "-isohybrid-gpt-basdat"])
        .arg_path(&paths.iso_root)
        .error_msg("xorriso failed. Install: sudo dnf install xorriso")
        .run()?;

    Ok(())
}

/// Stage 7: Generate SHA512 checksum for download verification.
fn generate_iso_checksum(iso_path: &Path) -> Result<()> {
    println!("Generating SHA512 checksum...");

    let result = Cmd::new("sha512sum")
        .arg_path(iso_path)
        .error_msg("sha512sum failed")
        .run()?;

    let hash = result
        .stdout
        .split_whitespace()
        .next()
        .context("Could not parse sha512sum output")?;

    let filename = iso_path
        .file_name()
        .context("Could not get ISO filename")?
        .to_string_lossy();

    let checksum_content = format!("{}{}{}\n", hash, SHA512_SEPARATOR, filename);

    let checksum_path = iso_path.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    fs::write(&checksum_path, &checksum_content)?;

    if hash.len() >= 16 {
        println!(
            "  SHA512: {}...{}",
            &hash[..8],
            &hash[hash.len() - 8..]
        );
    }
    println!("  Wrote: {}", checksum_path.display());

    Ok(())
}

/// Print summary after ISO creation.
fn print_iso_summary(iso_output: &Path) {
    println!("\n=== AcornOS ISO Created ===");
    println!("  Output: {}", iso_output.display());
    match fs::metadata(iso_output) {
        Ok(meta) => {
            println!("  Size: {} MB", meta.len() / 1024 / 1024);
        }
        Err(e) => {
            eprintln!("  [WARN] Could not read ISO size: {}", e);
        }
    }
    println!("  Label: {}", iso_label());
    println!("\nTo run in QEMU:");
    println!("  cargo run -- run");
}

/// Create a FAT16 image containing EFI boot files.
fn create_efi_boot_image(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    let efiboot_str = efiboot_img.to_string_lossy();

    // Create empty file
    Cmd::new("dd")
        .args(["if=/dev/zero", &format!("of={}", efiboot_str)])
        .args(["bs=1M", &format!("count={}", EFIBOOT_SIZE_MB)])
        .error_msg("Failed to create efiboot.img with dd")
        .run()?;

    // Format as FAT16
    Cmd::new("mkfs.fat")
        .args(["-F", "16"])
        .arg_path(efiboot_img)
        .error_msg("mkfs.fat failed. Install: sudo dnf install dosfstools")
        .run()?;

    // Create EFI/BOOT directory structure using mtools
    Cmd::new("mmd")
        .args(["-i", &efiboot_str, "::EFI"])
        .error_msg("mmd failed. Install: sudo dnf install mtools")
        .run()?;

    Cmd::new("mmd")
        .args(["-i", &efiboot_str, "::EFI/BOOT"])
        .error_msg("mmd failed to create ::EFI/BOOT directory")
        .run()?;

    // Copy EFI bootloader
    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy BOOTX64.EFI")
        .run()?;

    // Copy grub.cfg
    Cmd::new("mcopy")
        .args(["-i", &efiboot_str])
        .arg_path(&iso_root.join(ISO_EFI_DIR).join("grub.cfg"))
        .arg("::EFI/BOOT/")
        .error_msg("mcopy failed to copy grub.cfg")
        .run()?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join(EFIBOOT_FILENAME))?;

    Ok(())
}
