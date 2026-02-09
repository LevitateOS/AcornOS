//! QEMU runner for AcornOS.
//!
//! Thin wrapper over `distro_builder::qemu` with AcornOS-specific configuration.

use anyhow::{bail, Context, Result};
use std::path::Path;

use distro_builder::process::Cmd;
use distro_builder::qemu::{find_ovmf, QemuBuilder, SerialOutput};
use distro_spec::acorn::{
    ISO_FILENAME, QEMU_CPU_MODE, QEMU_DISK_FILENAME, QEMU_DISK_GB, QEMU_MEMORY_GB, QEMU_SERIAL_LOG,
};

/// Run the ISO in QEMU GUI.
pub fn run_iso(base_dir: &Path, disk_size: Option<String>) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join(ISO_FILENAME);

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'acornos iso' first.",
            iso_path.display()
        );
    }

    println!("Running ISO in QEMU GUI...");
    println!("  ISO: {}", iso_path.display());

    let kvm_available = std::path::Path::new("/dev/kvm").exists();
    if kvm_available {
        println!("  Acceleration: KVM (hardware virtualization)");
    } else {
        println!("  Acceleration: TCG (software emulation - slower)");
    }

    let mut builder = QemuBuilder::new(QEMU_CPU_MODE, QEMU_MEMORY_GB)
        .cdrom(iso_path.clone())
        .vga("virtio")
        .serial_output(SerialOutput::File(QEMU_SERIAL_LOG.to_string()));

    // Always include a virtual disk
    let size = disk_size.unwrap_or_else(|| format!("{}G", QEMU_DISK_GB));
    let disk_path = output_dir.join(QEMU_DISK_FILENAME);

    if !disk_path.exists() {
        println!("  Creating {} virtual disk...", size);
        Cmd::new("qemu-img")
            .args(["create", "-f", "qcow2"])
            .arg_path(&disk_path)
            .arg(&size)
            .error_msg("qemu-img create failed. Install: sudo dnf install qemu-img")
            .run()?;
    }

    println!("  Disk: {}", disk_path.display());
    builder = builder.disk(disk_path);

    let ovmf_path = find_ovmf().context(
        "OVMF firmware not found. AcornOS requires UEFI boot.\n\
         Install OVMF:\n\
         - Fedora/RHEL: sudo dnf install edk2-ovmf\n\
         - Debian/Ubuntu: sudo apt install ovmf\n\
         - Arch: sudo pacman -S edk2-ovmf",
    )?;

    println!("  Boot: UEFI ({})", ovmf_path.display());
    builder = builder.uefi(ovmf_path);

    let status = builder
        .build()
        .status()
        .context("Failed to run qemu-system-x86_64. Is QEMU installed?")?;

    if !status.success() {
        bail!("QEMU exited with status: {}", status);
    }

    Ok(())
}
