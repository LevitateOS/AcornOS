//! QEMU runner for AcornOS.
//!
//! Runs the AcornOS ISO in QEMU with UEFI boot.
//!
//! # Testing
//!
//! The `test_iso` function boots the ISO headless with serial console and
//! watches for success/failure patterns. This enables automated boot verification.

use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use distro_builder::process::Cmd;
use distro_spec::acorn::{
    ISO_FILENAME,
    QEMU_MEMORY_GB, QEMU_DISK_GB,
    QEMU_DISK_FILENAME, QEMU_SERIAL_LOG, QEMU_CPU_MODE,
};

/// Success patterns - if we see any of these, boot succeeded.
/// First entry (___SHELL_READY___) is the test instrumentation marker - preferred.
/// "login:" is the only fallback (indicates getty ready for interactive login).
/// Note: "AcornOS Live" is NOT a success pattern - it appears in /etc/issue
/// before the shell starts, and would match too early.
const SUCCESS_PATTERNS: &[&str] = &[
    "___SHELL_READY___",         // Test instrumentation - shell ready for commands
    "login:",                    // Getty login prompt (only appears without autologin)
];

/// Failure patterns - if we see any of these, boot failed.
const FAILURE_PATTERNS: &[&str] = &[
    "Kernel panic",
    "not syncing",
    "VFS: Cannot open root device",
    "No init found",
    "can't find /init",
    "EROFS error",
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
/// Returns Ok(()) if boot succeeds (login prompt reached) AND functional
/// verification passes (UEFI boot, PID 1, runlevel).
///
/// NOTE: This is a SMOKE TEST for quick developer feedback.
/// For comprehensive installation testing, use:
///   cd testing/install-tests && cargo run -- --distro acorn
pub fn test_iso(base_dir: &Path, timeout_secs: u64) -> Result<()> {
    let output_dir = base_dir.join("output");
    let iso_path = output_dir.join(ISO_FILENAME);

    if !iso_path.exists() {
        bail!(
            "ISO not found at {}. Run 'acornos iso' first.",
            iso_path.display()
        );
    }

    // =========================================================================
    // SMOKE TEST WARNING
    // =========================================================================
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                    SMOKE TEST - NOT FULL VERIFICATION             ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║ This test verifies:                                               ║");
    println!("║   ✓ UEFI boot (checks /sys/firmware/efi)                          ║");
    println!("║   ✓ PID 1 is init (not emergency shell)                           ║");
    println!("║   ✓ Default runlevel reached                                      ║");
    println!("║                                                                   ║");
    println!("║ For FULL installation testing:                                    ║");
    println!("║   cd testing/install-tests && cargo run -- --distro acorn         ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();

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

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    println!("Starting QEMU (headless, serial console)...\n");

    let mut child = cmd.spawn().context("Failed to spawn qemu-system-x86_64")?;
    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stdin = child.stdin.take().context("Failed to capture stdin")?;

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

                // Check for shell ready marker (test instrumentation)
                if line.contains("___SHELL_READY___") {
                    let boot_elapsed = start.elapsed().as_secs_f64();
                    println!();
                    println!("═══════════════════════════════════════════════════════════");
                    println!("SHELL READY: Test instrumentation active");
                    println!("═══════════════════════════════════════════════════════════");
                    println!();
                    println!("Boot completed in {:.1}s", boot_elapsed);
                    println!("Running functional verification...\n");

                    // Run functional verification
                    return run_functional_verification(
                        &mut child,
                        stdin,
                        &rx,
                        start,
                    );
                }

                // Check other success patterns (fallback if test instrumentation missing)
                for pattern in SUCCESS_PATTERNS.iter().skip(1) {  // Skip ___SHELL_READY___
                    if line.contains(pattern) {
                        let elapsed = start.elapsed().as_secs_f64();
                        let _ = child.kill();
                        let _ = child.wait();

                        println!();
                        println!("═══════════════════════════════════════════════════════════");
                        println!("BOOT DETECTED: Matched '{}'", pattern);
                        println!("═══════════════════════════════════════════════════════════");
                        println!();
                        println!("WARNING: Test instrumentation NOT detected!");
                        println!("         Functional verification SKIPPED.");
                        println!("         Check that profile/live-overlay is included in ISO.");
                        println!();
                        println!("Boot detected in {:.1}s (no verification)", elapsed);

                        // Return error since we can't verify
                        bail!(
                            "Boot detected but test instrumentation missing.\n\
                             Expected: ___SHELL_READY___ marker from /etc/profile.d/00-acorn-test.sh\n\
                             Got: '{}'\n\n\
                             This indicates the profile/live-overlay directory was not\n\
                             copied to the ISO. Rebuild with 'acornos iso' and try again.",
                            pattern
                        );
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

/// Run functional verification commands after shell is ready.
///
/// Verifies:
/// 1. UEFI boot (not -kernel bypass)
/// 2. PID 1 is init (not emergency shell)
/// 3. Default runlevel reached with services started
fn run_functional_verification(
    child: &mut Child,
    mut stdin: ChildStdin,
    rx: &Receiver<String>,
    start: Instant,
) -> Result<()> {
    // Helper to send command and collect response
    let send_cmd = |stdin: &mut ChildStdin, cmd: &str| -> Result<()> {
        writeln!(stdin, "{}", cmd)?;
        stdin.flush()?;
        Ok(())
    };

    // Helper to wait for response with timeout.
    // Note: We can't return early on ___PROMPT___ because serial console output
    // arrives in unpredictable chunks and UEFI_YES/NO might be on the same line
    // as the prompt or arrive after it due to buffering.
    let wait_response = |rx: &Receiver<String>, timeout_ms: u64| -> Vec<String> {
        let mut lines = Vec::new();
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    // Print verification output
                    println!("  [verify] {}", line);
                    lines.push(line);
                }
                Err(_) => continue,
            }
        }
        lines
    };

    // =========================================================================
    // Verification 1: UEFI Boot
    // =========================================================================
    println!("Verifying UEFI boot...");
    // Use unique markers that don't appear in the command itself
    send_cmd(&mut stdin, "test -d /sys/firmware/efi && echo UEFI_YES || echo UEFI_NO")?;
    let response = wait_response(rx, 2000);

    // Only check lines that are exactly UEFI_YES or UEFI_NO (not echoed command)
    let uefi_ok = response.iter().any(|l| l.trim() == "UEFI_YES");
    let no_uefi = response.iter().any(|l| l.trim() == "UEFI_NO");

    if no_uefi && !uefi_ok {
        let _ = child.kill();
        bail!(
            "UEFI VERIFICATION FAILED\n\
             Expected: /sys/firmware/efi directory present\n\
             Got: Directory not found\n\n\
             This means QEMU booted via -kernel bypass, not UEFI firmware.\n\
             The test does not reflect real hardware boot behavior."
        );
    }
    if !uefi_ok && !no_uefi {
        // Neither marker found - inconclusive result
        let _ = child.kill();
        bail!(
            "UEFI VERIFICATION INCONCLUSIVE\n\
             Expected: UEFI_YES or UEFI_NO response\n\
             Got: {:?}\n\n\
             Serial I/O may be broken or command timed out.",
            response
        );
    }
    println!("  ✓ UEFI boot confirmed\n");

    // =========================================================================
    // Verification 2: PID 1
    // =========================================================================
    println!("Verifying PID 1...");
    send_cmd(&mut stdin, "cat /proc/1/comm")?;
    let response = wait_response(rx, 2000);
    let pid1_ok = response.iter().any(|l| l.contains("init"));

    if !pid1_ok {
        let _ = child.kill();
        let pid1_name = response.iter()
            .find(|l| !l.contains("cat") && !l.contains("___"))
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        bail!(
            "PID 1 VERIFICATION FAILED\n\
             Expected: init\n\
             Got: {}\n\n\
             The system may be in emergency shell or recovery mode.",
            pid1_name
        );
    }
    println!("  ✓ PID 1 is init\n");

    // =========================================================================
    // Verification 3: Default Runlevel
    // =========================================================================
    println!("Verifying default runlevel...");
    send_cmd(&mut stdin, "rc-status default 2>/dev/null | grep -c started || echo 0")?;
    let response = wait_response(rx, 3000);

    // Look for a number in the response (count of started services)
    let started_count: u32 = response.iter()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .next()
        .unwrap_or(0);

    if started_count == 0 {
        let _ = child.kill();
        bail!(
            "RUNLEVEL VERIFICATION FAILED\n\
             Expected: At least 1 service started in default runlevel\n\
             Got: 0 services started\n\n\
             OpenRC may not have reached the default runlevel."
        );
    }
    println!("  ✓ Default runlevel reached ({} services started)\n", started_count);

    // =========================================================================
    // Verification 4: Check for crashed services
    // =========================================================================
    println!("Checking for crashed services...");
    // Use tail -n +2 to skip the "Crashed services:" header line, then count non-empty lines
    send_cmd(&mut stdin, "rc-status --crashed 2>/dev/null | tail -n +2 | grep -c . || echo 0")?;
    let response = wait_response(rx, 2000);

    let crashed_count: u32 = response.iter()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .next()
        .unwrap_or(0);

    if crashed_count > 0 {
        let _ = child.kill();
        bail!(
            "CRASHED SERVICES DETECTED\n\
             Found {} crashed service(s)\n\n\
             Run 'rc-status --crashed' manually to investigate.",
            crashed_count
        );
    }
    println!("  ✓ No crashed services\n");

    // =========================================================================
    // All verifications passed
    // =========================================================================
    let total_elapsed = start.elapsed().as_secs_f64();
    let _ = child.kill();
    let _ = child.wait();

    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                    SMOKE TEST PASSED                              ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║ Verified:                                                         ║");
    println!("║   ✓ UEFI boot (not -kernel bypass)                                ║");
    println!("║   ✓ PID 1 is init (not emergency shell)                           ║");
    println!("║   ✓ Default runlevel reached                                      ║");
    println!("║   ✓ No crashed services                                           ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║ Total time: {:.1}s                                                ║", total_elapsed);
    println!("║                                                                   ║");
    println!("║ For FULL installation testing:                                    ║");
    println!("║   cd testing/install-tests && cargo run -- --distro acorn         ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");

    Ok(())
}
