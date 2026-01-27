//! ISO creation - builds bootable AcornOS ISO.
//!
//! Creates an ISO with EROFS-based architecture:
//! - Tiny initramfs (~5MB) - mounts EROFS + overlay
//! - EROFS image (~200MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Use shared infrastructure from distro-builder
use distro_builder::artifact::filesystem::{atomic_move, copy_dir_recursive};
use distro_builder::artifact::iso_utils::{
    create_efi_dirs_in_fat, create_fat16_image, generate_iso_checksum, mcopy_to_fat, run_xorriso,
    setup_iso_structure,
};
use distro_builder::process::Cmd;
use distro_spec::acorn::{
    // Identity
    ISO_FILENAME, ISO_LABEL, OS_NAME,
    // EROFS rootfs
    ROOTFS_ISO_PATH, ROOTFS_NAME,
    // Boot files
    INITRAMFS_LIVE_ISO_PATH, INITRAMFS_LIVE_OUTPUT, KERNEL_ISO_PATH,
    // ISO structure
    ISO_EFI_DIR, LIVE_OVERLAY_ISO_PATH,
    // EFI
    EFIBOOT_FILENAME, EFIBOOT_SIZE_MB, EFI_BOOTLOADER,
    // Console
    SERIAL_CONSOLE, VGA_CONSOLE,
    // Checksum
    ISO_CHECKSUM_SUFFIX,
};

/// Get ISO volume label from environment or use default.
fn iso_label() -> String {
    env::var("ISO_LABEL").unwrap_or_else(|_| ISO_LABEL.to_string())
}

/// Paths used during ISO creation.
struct IsoPaths {
    output_dir: PathBuf,
    rootfs: PathBuf,
    initramfs_live: PathBuf,
    iso_output: PathBuf,
    iso_root: PathBuf,
    /// Custom-built kernel at output/staging/boot/vmlinuz
    kernel: PathBuf,
}

impl IsoPaths {
    fn new(base_dir: &Path) -> Self {
        let output_dir = base_dir.join("output");
        Self {
            rootfs: output_dir.join(ROOTFS_NAME),
            initramfs_live: output_dir.join(INITRAMFS_LIVE_OUTPUT),
            iso_output: output_dir.join(ISO_FILENAME),
            iso_root: output_dir.join("iso-root"),
            // Custom kernel from our build (stolen from leviso or built ourselves)
            kernel: output_dir.join("staging/boot/vmlinuz"),
            output_dir,
        }
    }
}

/// Create ISO using EROFS-based architecture.
///
/// This creates an ISO with:
/// - Tiny initramfs (~5MB) - mounts EROFS + overlay
/// - EROFS image (~200MB) - complete base system
/// - Live overlay - live-specific configs (autologin, serial console)
///
/// Boot flow:
/// 1. kernel -> tiny initramfs
/// 2. init_tiny mounts EROFS as lower layer
/// 3. init_tiny mounts /live/overlay from ISO as middle layer
/// 4. init_tiny mounts tmpfs as upper layer (for writes)
/// 5. switch_root -> OpenRC
pub fn create_iso(base_dir: &Path) -> Result<()> {
    let paths = IsoPaths::new(base_dir);

    println!("=== Building AcornOS ISO ===\n");

    // Stage 1: Validate inputs
    validate_iso_inputs(&paths)?;

    // Stage 2: Create live overlay (autologin, serial console, empty root password)
    create_live_overlay(&paths.output_dir)?;

    // Stage 3: Set up ISO directory structure (using shared infrastructure)
    setup_iso_structure(&paths.iso_root)?;

    // Stage 4: Copy boot files and artifacts
    copy_iso_artifacts(&paths)?;

    // Stage 5: Set up UEFI boot
    setup_uefi_boot(&paths)?;

    // Stage 6: Create the ISO to a temporary file (using shared infrastructure)
    let temp_iso = paths.output_dir.join(format!("{}.tmp", ISO_FILENAME));
    let label = iso_label();
    run_xorriso(&paths.iso_root, &temp_iso, &label, EFIBOOT_FILENAME)?;

    // Stage 7: Generate checksum for the temporary ISO (using shared infrastructure)
    let _checksum_path = generate_iso_checksum(&temp_iso)?;

    // Atomic move to final destination (with cross-filesystem fallback)
    atomic_move(&temp_iso, &paths.iso_output)?;

    // Also move the checksum
    let temp_checksum = temp_iso.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let final_checksum = paths.iso_output.with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
    let _ = atomic_move(&temp_checksum, &final_checksum);

    print_iso_summary(&paths.iso_output);
    Ok(())
}

/// Stage 1: Validate that required input files exist.
fn validate_iso_inputs(paths: &IsoPaths) -> Result<()> {
    if !paths.rootfs.exists() {
        bail!(
            "EROFS rootfs not found at {}.\n\
             Run 'acornos build rootfs' first.",
            paths.rootfs.display()
        );
    }

    if !paths.initramfs_live.exists() {
        bail!(
            "Live initramfs not found at {}.\n\
             Run 'acornos initramfs' first.",
            paths.initramfs_live.display()
        );
    }

    // Check for custom-built kernel
    if !paths.kernel.exists() {
        bail!(
            "Kernel not found at {}.\n\
             Run 'acornos build kernel' first.",
            paths.kernel.display()
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
        copy_dir_recursive(&profile_overlay, &live_overlay)
            .with_context(|| format!("Failed to copy {} -> {}", profile_overlay.display(), live_overlay.display()))?;
    }

    // =========================================================================
    // STEP 2: Code-generated overlay files (may override profile defaults)
    // =========================================================================

    // Create directory structure
    fs::create_dir_all(live_overlay.join("etc"))
        .with_context(|| "Failed to create etc")?;

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
    // The EROFS base system has a locked root password
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
    // This overlays the base inittab from EROFS (which has serial commented out)
    //
    // For autologin on serial, we use agetty --autologin which:
    // - Automatically logs in the specified user (root)
    // - Spawns a login shell which sources /etc/profile and /etc/profile.d/*
    // - This is the standard Alpine Linux approach (see wiki.alpinelinux.org/wiki/TTY_Autologin)
    // Serial console uses inittab directly since Alpine Extended doesn't include
    // the openrc-agetty package that provides /etc/init.d/agetty
    let inittab_content = r#"# /etc/inittab - AcornOS Live

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

# Serial console with autologin for test harness
# Uses wrapper script that spawns ash as login shell (sources /etc/profile.d/*)
ttyS0::respawn:/sbin/getty -n -l /usr/local/bin/serial-autologin 115200 ttyS0 vt100

# Ctrl+Alt+Del
::ctrlaltdel:/sbin/reboot

# Shutdown
::shutdown:/sbin/openrc shutdown
"#;
    fs::write(live_overlay.join("etc/inittab"), inittab_content)?;

    // Ensure runlevels directory exists for services enabled via profile/live-overlay
    let runlevels_default = live_overlay.join("etc/runlevels/default");
    fs::create_dir_all(&runlevels_default)?;

    // NOTE: Serial console is handled via inittab, not OpenRC service,
    // since Alpine Extended doesn't include the openrc-agetty package.

    // Create conf.d directory for future service configuration
    let conf_d = live_overlay.join("etc/conf.d");
    fs::create_dir_all(&conf_d)?;

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

    // Mount efivarfs if not already mounted (needed for bootctl, efibootmgr)
    // The initramfs mount might not persist through switch_root properly
    let efivars_script = r#"#!/bin/sh
# Ensure efivarfs is mounted for UEFI support
# Needed for efibootmgr, bootctl, and install tests

if [ -d /sys/firmware/efi ]; then
    mkdir -p /sys/firmware/efi/efivars 2>/dev/null
    mount -t efivarfs efivarfs /sys/firmware/efi/efivars 2>/dev/null || true
fi
"#;
    let efivars_script_path = live_overlay.join("etc/local.d/01-efivarfs.start");
    fs::write(&efivars_script_path, efivars_script)?;
    let mut perms = fs::metadata(&efivars_script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&efivars_script_path, perms)?;

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

/// Stage 4: Copy kernel, initramfs, rootfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths) -> Result<()> {
    // Copy custom-built kernel (from output/staging/boot/vmlinuz)
    println!("Copying kernel from {}...", paths.kernel.display());
    fs::copy(&paths.kernel, paths.iso_root.join(KERNEL_ISO_PATH))?;

    // Copy live initramfs
    println!("Copying initramfs...");
    fs::copy(&paths.initramfs_live, paths.iso_root.join(INITRAMFS_LIVE_ISO_PATH))?;

    // Copy EROFS rootfs to /live/
    println!("Copying EROFS rootfs to ISO...");
    fs::copy(&paths.rootfs, paths.iso_root.join(ROOTFS_ISO_PATH))?;

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
        let grub_cmd = if distro_builder::process::exists("grub2-mkstandalone") {
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
    // Modules needed for live boot (EROFS + overlay)
    let modules = "modules=loop,erofs,overlay,virtio_pci,virtio_blk,virtio_scsi,sd-mod,sr-mod,cdrom,isofs";
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

/// Create a FAT16 image containing EFI boot files (using shared infrastructure).
fn create_efi_boot_image(iso_root: &Path, efiboot_img: &Path) -> Result<()> {
    // Use shared utilities from distro-builder
    create_fat16_image(efiboot_img, EFIBOOT_SIZE_MB)?;
    create_efi_dirs_in_fat(efiboot_img)?;

    // Copy EFI bootloader and grub.cfg
    mcopy_to_fat(
        efiboot_img,
        &iso_root.join(ISO_EFI_DIR).join(EFI_BOOTLOADER),
        "::EFI/BOOT/",
    )?;
    mcopy_to_fat(
        efiboot_img,
        &iso_root.join(ISO_EFI_DIR).join("grub.cfg"),
        "::EFI/BOOT/",
    )?;

    // Copy efiboot.img into iso-root for xorriso
    fs::copy(efiboot_img, iso_root.join(EFIBOOT_FILENAME))?;

    Ok(())
}
