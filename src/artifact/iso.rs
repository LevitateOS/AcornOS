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

    // Create directory structure
    fs::create_dir_all(live_overlay.join("etc"))?;

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
    // Create /etc/inittab addition for serial getty
    fs::create_dir_all(live_overlay.join("etc/init.d"))?;

    // OpenRC uses agetty via inittab or runlevel scripts
    // Create a simple override for the inittab
    let inittab_content = "# Serial console for live boot testing\n\
                           ttyS0::respawn:/sbin/getty -L ttyS0 115200 vt100\n";
    fs::create_dir_all(live_overlay.join("etc/local.d"))?;
    fs::write(
        live_overlay.join("etc/inittab.live"),
        inittab_content,
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

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
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
        OS_NAME, KERNEL_ISO_PATH, modules, label, SERIAL_CONSOLE, VGA_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, modules, label, SERIAL_CONSOLE, VGA_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
        OS_NAME, KERNEL_ISO_PATH, modules, label, SERIAL_CONSOLE, VGA_CONSOLE, INITRAMFS_LIVE_ISO_PATH,
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
