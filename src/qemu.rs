//! QEMU runner for AcornOS.
//!
//! Runs the AcornOS ISO in QEMU with UEFI boot.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use distro_builder::process::Cmd;
use distro_spec::acorn::{
    ISO_FILENAME,
    QEMU_MEMORY_GB, QEMU_DISK_GB,
    QEMU_DISK_FILENAME, QEMU_SERIAL_LOG, QEMU_CPU_MODE,
};

/// Builder for QEMU commands.
#[derive(Default)]
struct QemuBuilder {
    cdrom: Option<PathBuf>,
    disk: Option<PathBuf>,
    ovmf: Option<PathBuf>,
    vga: Option<String>,
}

impl QemuBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn cdrom(mut self, path: PathBuf) -> Self {
        self.cdrom = Some(path);
        self
    }

    fn disk(mut self, path: PathBuf) -> Self {
        self.disk = Some(path);
        self
    }

    fn uefi(mut self, ovmf_path: PathBuf) -> Self {
        self.ovmf = Some(ovmf_path);
        self
    }

    fn vga(mut self, vga_type: &str) -> Self {
        self.vga = Some(vga_type.to_string());
        self
    }

    fn build(self) -> Command {
        let mut cmd = Command::new("qemu-system-x86_64");

        // Enable KVM acceleration if available
        let kvm_available = std::path::Path::new("/dev/kvm").exists();
        if kvm_available {
            cmd.args(["-enable-kvm", "-cpu", "host"]);
        } else {
            cmd.args(["-cpu", QEMU_CPU_MODE]);
        }

        // SMP: 4 cores for reasonable performance
        cmd.args(["-smp", "4"]);

        // Memory (4G - AcornOS is a daily driver OS)
        cmd.args(["-m", &format!("{}G", QEMU_MEMORY_GB)]);

        // CD-ROM (use virtio-scsi for better compatibility)
        if let Some(cdrom) = &self.cdrom {
            cmd.args([
                "-device", "virtio-scsi-pci,id=scsi0",
                "-device", "scsi-cd,drive=cdrom0,bus=scsi0.0",
                "-drive", &format!("id=cdrom0,if=none,format=raw,readonly=on,file={}", cdrom.display()),
            ]);
        }

        // Virtio disk
        if let Some(disk) = &self.disk {
            cmd.args([
                "-drive",
                &format!("file={},format=qcow2,if=virtio", disk.display()),
            ]);
        }

        // UEFI firmware
        if let Some(ovmf) = &self.ovmf {
            cmd.args([
                "-drive",
                &format!("if=pflash,format=raw,readonly=on,file={}", ovmf.display()),
            ]);
        }

        // Network: virtio-net with user-mode NAT
        cmd.args([
            "-netdev", "user,id=net0",
            "-device", "virtio-net-pci,netdev=net0",
        ]);

        // Display options
        if let Some(vga) = &self.vga {
            if vga == "virtio" {
                cmd.args([
                    "-display", "gtk,gl=on",
                    "-device", "virtio-gpu-gl,xres=1920,yres=1080",
                ]);
            } else {
                cmd.args(["-vga", vga]);
            }
            // Add serial port for kernel messages
            cmd.args(["-serial", &format!("file:{}", QEMU_SERIAL_LOG)]);
        }

        cmd
    }
}

/// Find OVMF firmware for UEFI boot.
fn find_ovmf() -> Option<PathBuf> {
    let candidates = [
        // Fedora/RHEL
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        // Debian/Ubuntu
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/qemu/OVMF.fd",
        // Arch
        "/usr/share/edk2-ovmf/x64/OVMF_CODE.fd",
        // NixOS
        "/run/libvirt/nix-ovmf/OVMF_CODE.fd",
    ];

    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

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

    // Check KVM availability
    let kvm_available = std::path::Path::new("/dev/kvm").exists();
    if kvm_available {
        println!("  Acceleration: KVM (hardware virtualization)");
    } else {
        println!("  Acceleration: TCG (software emulation - slower)");
    }

    let mut builder = QemuBuilder::new().cdrom(iso_path.clone()).vga("virtio");

    // Always include a virtual disk
    let size = disk_size.unwrap_or_else(|| format!("{}G", QEMU_DISK_GB));
    let disk_path = output_dir.join(QEMU_DISK_FILENAME);

    // Create disk if it doesn't exist
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

    // AcornOS requires UEFI boot
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
