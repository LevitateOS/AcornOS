//! QEMU runner for AcornOS.
//!
//! Runs the AcornOS ISO in QEMU with UEFI boot.
//!
//! # Testing
//!
//! The `test_iso` function boots the ISO headless with serial console and
//! watches for success/failure patterns. This enables automated boot verification.

use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use distro_builder::process::Cmd;
use distro_spec::acorn::{
    ISO_FILENAME,
    QEMU_MEMORY_GB, QEMU_DISK_GB,
    QEMU_DISK_FILENAME, QEMU_SERIAL_LOG, QEMU_CPU_MODE,
};

/// Success patterns - if we see any of these, boot succeeded.
const SUCCESS_PATTERNS: &[&str] = &[
    "login:",                    // Getty prompt - definitive success
    "AcornOS Live",              // Our /etc/issue - getty is showing login
    "Login as 'root'",           // Our /etc/issue line
];

/// Failure patterns - if we see any of these, boot failed.
const FAILURE_PATTERNS: &[&str] = &[
    "Kernel panic",
    "not syncing",
    "VFS: Cannot open root device",
    "No init found",
    "can't find /init",
    "SQUASHFS error",
    "failed to mount",
    "emergency shell",
    "No bootable device",
    "Boot Failed",
];

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

        // CD-ROM (use AHCI for consistency with LevitateOS/real hardware)
        if let Some(cdrom) = &self.cdrom {
            cmd.args([
                "-device", "ahci,id=ahci0",
                "-device", "ide-cd,drive=cdrom0,bus=ahci0.0",
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

/// Test the ISO by booting headless and watching serial output.
///
/// Returns Ok(()) if boot succeeds (login prompt reached).
/// Returns Err if boot fails or times out.
pub fn test_iso(base_dir: &Path, timeout_secs: u64) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join(ISO_FILENAME);

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'acornos iso' first.",
            iso_path.display()
        );
    }

    println!("=== AcornOS Boot Test ===\n");
    println!("ISO: {}", iso_path.display());
    println!("Timeout: {}s", timeout_secs);
    println!();

    // Find OVMF
    let ovmf_path = find_ovmf().context("OVMF not found - UEFI boot required")?;

    // Build headless QEMU command with serial console
    let mut cmd = Command::new("qemu-system-x86_64");

    // Enable KVM if available
    let kvm_available = std::path::Path::new("/dev/kvm").exists();
    if kvm_available {
        cmd.args(["-enable-kvm", "-cpu", "host"]);
    } else {
        cmd.args(["-cpu", QEMU_CPU_MODE]);
    }

    cmd.args(["-smp", "2"]);
    cmd.args(["-m", &format!("{}G", QEMU_MEMORY_GB)]);

    // CD-ROM via AHCI (consistency with LevitateOS/real hardware)
    cmd.args([
        "-device", "ahci,id=ahci0",
        "-device", "ide-cd,drive=cdrom0,bus=ahci0.0",
        "-drive", &format!("id=cdrom0,if=none,format=raw,readonly=on,file={}", iso_path.display()),
    ]);

    // UEFI firmware
    cmd.args([
        "-drive",
        &format!("if=pflash,format=raw,readonly=on,file={}", ovmf_path.display()),
    ]);

    // Headless with serial console
    cmd.args(["-nographic", "-serial", "mon:stdio", "-no-reboot"]);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    println!("Starting QEMU (headless, serial console)...\n");

    let mut child = cmd.spawn().context("Failed to spawn qemu-system-x86_64")?;
    let stdout = child.stdout.take().context("Failed to capture stdout")?;

    // Spawn reader thread
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Watch for patterns
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let stall_timeout = Duration::from_secs(30); // No output for 30s = stall
    let mut last_output = Instant::now();
    let mut output_buffer: Vec<String> = Vec::new();

    // Boot stage tracking
    let mut saw_uefi = false;
    let mut saw_kernel = false;
    let mut saw_init = false;

    println!("Watching boot output...\n");

    loop {
        // Check overall timeout
        if start.elapsed() > timeout {
            let _ = child.kill();
            let last_lines = output_buffer.iter().rev().take(20).cloned().collect::<Vec<_>>();
            bail!(
                "TIMEOUT: Boot did not complete in {}s\n\nLast output:\n{}",
                timeout_secs,
                last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
            );
        }

        // Check stall
        if last_output.elapsed() > stall_timeout {
            let _ = child.kill();
            let stage = if saw_init {
                "Init started but stalled"
            } else if saw_kernel {
                "Kernel started but init stalled"
            } else if saw_uefi {
                "UEFI ran but kernel stalled"
            } else {
                "No output - QEMU/serial broken"
            };
            bail!("STALL: {} (no output for {}s)", stage, stall_timeout.as_secs());
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                last_output = Instant::now();
                output_buffer.push(line.clone());

                // Print output for visibility
                println!("  {}", line);

                // Track boot stages
                if line.contains("UEFI") || line.contains("EFI") || line.contains("BdsDxe") {
                    saw_uefi = true;
                }
                if line.contains("Linux version") || line.contains("Booting Linux") {
                    saw_kernel = true;
                }
                if line.contains("OpenRC") || line.contains("init") {
                    saw_init = true;
                }

                // Check failure patterns first (fail fast)
                for pattern in FAILURE_PATTERNS {
                    if line.contains(pattern) {
                        let _ = child.kill();
                        let last_lines = output_buffer.iter().rev().take(30).cloned().collect::<Vec<_>>();
                        bail!(
                            "BOOT FAILED: {}\n\nContext:\n{}",
                            pattern,
                            last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                        );
                    }
                }

                // Check success patterns
                for pattern in SUCCESS_PATTERNS {
                    if line.contains(pattern) {
                        let elapsed = start.elapsed().as_secs_f64();
                        let _ = child.kill();
                        let _ = child.wait();

                        println!();
                        println!("═══════════════════════════════════════════════════════════");
                        println!("BOOT SUCCESS: Matched '{}'", pattern);
                        println!("═══════════════════════════════════════════════════════════");
                        println!();
                        println!("Boot completed in {:.1}s", elapsed);
                        println!("AcornOS is ready for login.");

                        return Ok(());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let last_lines = output_buffer.iter().rev().take(20).cloned().collect::<Vec<_>>();
                bail!(
                    "QEMU exited unexpectedly\n\nLast output:\n{}",
                    last_lines.into_iter().rev().collect::<Vec<_>>().join("\n")
                );
            }
        }
    }
}
