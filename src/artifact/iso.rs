//! ISO creation - builds bootable AcornOS ISO.
//!
//! Creates an ISO with EROFS-based architecture:
//! - Tiny initramfs (~5MB) - mounts EROFS + overlay
//! - EROFS image (~200MB) - complete base system
//! - Live overlay - live-specific configs (autologin, serial console, empty root password)

use anyhow::{bail, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Use shared infrastructure from distro-builder
use distro_builder::artifact::filesystem::{atomic_move, copy_dir_recursive};
use distro_builder::artifact::iso_utils::{
    create_efi_dirs_in_fat, create_fat16_image, generate_iso_checksum, mcopy_to_fat, run_xorriso,
    setup_iso_structure,
};
use distro_builder::artifact::live_overlay::{
    create_openrc_live_overlay, InittabVariant, LiveOverlayConfig,
};
use distro_builder::process::Cmd;
use distro_spec::acorn::{
    default_loader_config,
    // EFI
    EFIBOOT_FILENAME,
    EFIBOOT_SIZE_MB,
    EFI_BOOTLOADER,
    // Boot files
    INITRAMFS_LIVE_ISO_PATH,
    INITRAMFS_LIVE_OUTPUT,
    // Checksum
    ISO_CHECKSUM_SUFFIX,
    // ISO structure
    ISO_EFI_DIR,
    // Identity
    ISO_FILENAME,
    ISO_LABEL,
    KERNEL_ISO_PATH,
    LIVE_OVERLAY_ISO_PATH,
    OS_NAME,
    // EROFS rootfs
    ROOTFS_ISO_PATH,
    ROOTFS_NAME,
    // Console
    SERIAL_CONSOLE,
    // UKI
    UKI_EFI_DIR,
    VGA_CONSOLE,
};

use super::uki::build_live_ukis;

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

    // Stage 4.5: Build UKIs (Unified Kernel Images)
    let uki_dir = paths.iso_root.join(UKI_EFI_DIR);
    fs::create_dir_all(&uki_dir)?;
    build_live_ukis(&paths.kernel, &paths.initramfs_live, &uki_dir)?;

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
    let final_checksum = paths
        .iso_output
        .with_extension(ISO_CHECKSUM_SUFFIX.trim_start_matches('.'));
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

/// Create live overlay using shared infrastructure.
fn create_live_overlay(output_dir: &Path) -> Result<()> {
    let base_dir = output_dir.parent().unwrap_or(Path::new("."));
    let profile_overlay = base_dir.join("profile/live-overlay");

    let config = LiveOverlayConfig {
        os_name: OS_NAME,
        inittab: InittabVariant::DesktopWithSerial,
        profile_overlay: if profile_overlay.exists() {
            Some(profile_overlay.as_path())
        } else {
            None
        },
    };

    create_openrc_live_overlay(output_dir, &config)?;
    Ok(())
}

/// Stage 4: Copy kernel, initramfs, rootfs, and live overlay to ISO.
fn copy_iso_artifacts(paths: &IsoPaths) -> Result<()> {
    // Copy custom-built kernel (from output/staging/boot/vmlinuz)
    println!("Copying kernel from {}...", paths.kernel.display());
    fs::copy(&paths.kernel, paths.iso_root.join(KERNEL_ISO_PATH))?;

    // Copy live initramfs
    println!("Copying initramfs...");
    fs::copy(
        &paths.initramfs_live,
        paths.iso_root.join(INITRAMFS_LIVE_ISO_PATH),
    )?;

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
    let iso_contents = paths
        .output_dir
        .parent()
        .unwrap()
        .join("downloads/iso-contents");
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
            .arg(format!(
                "boot/grub/grub.cfg={}",
                embedded_cfg_path.display()
            ))
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
    let modules =
        "modules=loop,erofs,overlay,virtio_pci,virtio_blk,virtio_scsi,sd-mod,sr-mod,cdrom,isofs";
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
        OS_NAME,
        KERNEL_ISO_PATH,
        modules,
        label,
        VGA_CONSOLE,
        SERIAL_CONSOLE,
        INITRAMFS_LIVE_ISO_PATH,
        OS_NAME,
        KERNEL_ISO_PATH,
        modules,
        label,
        VGA_CONSOLE,
        SERIAL_CONSOLE,
        INITRAMFS_LIVE_ISO_PATH,
        OS_NAME,
        KERNEL_ISO_PATH,
        modules,
        label,
        VGA_CONSOLE,
        SERIAL_CONSOLE,
        INITRAMFS_LIVE_ISO_PATH,
    );

    // Write to both locations for compatibility:
    // - /boot/grub/grub.cfg (where Alpine's bootloader looks)
    // - /EFI/BOOT/grub.cfg (standard EFI location)
    let boot_grub = paths.iso_root.join("boot/grub");
    fs::create_dir_all(&boot_grub)?;
    fs::write(boot_grub.join("grub.cfg"), &grub_cfg)?;
    fs::write(paths.iso_root.join(ISO_EFI_DIR).join("grub.cfg"), &grub_cfg)?;

    // Create systemd-boot loader.conf
    // This is placed in the EFI boot image for automatic discovery by systemd-boot
    println!("  Creating systemd-boot loader.conf...");
    let loader_config = default_loader_config();
    let loader_conf_content = loader_config.to_loader_conf();
    let loader_dir = paths.iso_root.join(ISO_EFI_DIR).join("loader");
    fs::create_dir_all(&loader_dir)?;
    fs::write(loader_dir.join("loader.conf"), &loader_conf_content)?;

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
