//! AcornOS ISO Builder CLI
//!
//! Builds AcornOS: a daily driver Linux distribution using musl, busybox, and OpenRC.
//! Packages are sourced from Alpine Linux repositories (APKs).
//!
//! # Usage
//!
//! ```bash
//! # Show current status
//! acornos status
//!
//! # Download Alpine Extended ISO (~1GB)
//! recipe resolve alpine
//!
//! # Build squashfs only
//! acornos build squashfs
//!
//! # Build complete ISO (squashfs + initramfs + ISO)
//! acornos build
//!
//! # Rebuild only the initramfs
//! acornos initramfs
//!
//! # Rebuild only the ISO
//! acornos iso
//!
//! # Run in QEMU
//! acornos run
//! ```
//!
//! # Differences from LevitateOS (leviso)
//!
//! | Aspect | LevitateOS | AcornOS |
//! |--------|-----------|---------|
//! | Base | Rocky Linux RPMs | Alpine APKs |
//! | Init | systemd | OpenRC |
//! | libc | glibc | musl |
//! | Coreutils | GNU | busybox |
//! | Shell | bash | ash (busybox) |

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "acornos")]
#[command(author, version, about = "AcornOS ISO builder", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build artifacts (squashfs, or full build)
    Build {
        #[command(subcommand)]
        artifact: Option<BuildArtifact>,
    },

    /// Rebuild only the initramfs
    Initramfs,

    /// Rebuild only the ISO (requires squashfs and initramfs)
    Iso,

    /// Run the ISO in QEMU (GUI)
    Run,

    /// Test the ISO boots correctly (headless, automated)
    Test {
        /// Timeout in seconds (default: 120)
        #[arg(short, long, default_value = "120")]
        timeout: u64,
    },

    /// Show build status and next steps
    Status,
}

#[derive(Subcommand)]
enum BuildArtifact {
    /// Build the kernel from source
    Kernel {
        /// Clean build directory before building
        #[arg(long)]
        clean: bool,
    },
    /// Build only the squashfs from rootfs
    Squashfs,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { artifact } => match artifact {
            Some(BuildArtifact::Kernel { clean }) => cmd_build_kernel(clean),
            Some(BuildArtifact::Squashfs) => cmd_build_squashfs(),
            None => cmd_build(),
        },
        Commands::Initramfs => cmd_initramfs(),
        Commands::Iso => cmd_iso(),
        Commands::Run => cmd_run(),
        Commands::Test { timeout } => cmd_test(timeout),
        Commands::Status => cmd_status(),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

fn cmd_build() -> Result<()> {
    use std::time::Instant;
    use acornos::Timer;

    // Full build: kernel + squashfs + initramfs + ISO
    // Skips anything already built, rebuilds only on changes.
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = base_dir.parent().expect("AcornOS must be in workspace");
    let linux_source = workspace_root.join("linux");
    let build_start = Instant::now();

    println!("=== Full AcornOS Build ===\n");

    // 1. Build kernel (skip if inputs unchanged)
    let needs_compile = acornos::rebuild::kernel_needs_compile(&base_dir);
    let needs_install = acornos::rebuild::kernel_needs_install(&base_dir);
    let output_dir = base_dir.join("output");

    if needs_compile {
        if !linux_source.exists() || !linux_source.join("Makefile").exists() {
            anyhow::bail!(
                "Linux kernel source not found at {}\n\
                 Run: git submodule update --init linux",
                linux_source.display()
            );
        }
        println!("Building kernel...");
        let t = Timer::start("Kernel");
        acornos::build::kernel::build_kernel(&linux_source, &output_dir, &base_dir)?;
        acornos::build::kernel::install_kernel(&linux_source, &output_dir, &output_dir.join("staging"))?;
        acornos::rebuild::cache_kernel_hash(&base_dir);
        t.finish();
    } else if needs_install {
        println!("Installing kernel (compile skipped)...");
        let t = Timer::start("Kernel install");
        acornos::build::kernel::install_kernel(&linux_source, &output_dir, &output_dir.join("staging"))?;
        t.finish();
    } else {
        println!("[SKIP] Kernel already built and installed");
    }

    // 2. Build squashfs (skip if inputs unchanged)
    if acornos::rebuild::squashfs_needs_rebuild(&base_dir) {
        println!("\nBuilding squashfs system image...");
        let t = Timer::start("Squashfs");
        acornos::artifact::build_squashfs(&base_dir)?;
        acornos::rebuild::cache_squashfs_hash(&base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Squashfs already built (inputs unchanged)");
    }

    // 3. Build initramfs (skip if inputs unchanged)
    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        println!("\nBuilding tiny initramfs...");
        let t = Timer::start("Initramfs");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 4. Build ISO (skip if components unchanged)
    if acornos::rebuild::iso_needs_rebuild(&base_dir) {
        println!("\nBuilding ISO...");
        let t = Timer::start("ISO");
        acornos::artifact::create_squashfs_iso(&base_dir)?;
        t.finish();
    } else {
        println!("\n[SKIP] ISO already built (components unchanged)");
    }

    let total = build_start.elapsed().as_secs_f64();
    if total >= 60.0 {
        println!("\n=== Build Complete ({:.1}m) ===", total / 60.0);
    } else {
        println!("\n=== Build Complete ({:.1}s) ===", total);
    }
    println!("  ISO: output/acornos.iso");
    println!("\nNext: acornos run");

    Ok(())
}

fn cmd_build_kernel(clean: bool) -> Result<()> {
    use acornos::Timer;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Linux kernel source is at the workspace root (shared with leviso)
    let workspace_root = base_dir.parent().expect("AcornOS must be in workspace");
    let linux_source = workspace_root.join("linux");

    if !linux_source.exists() || !linux_source.join("Makefile").exists() {
        anyhow::bail!(
            "Linux kernel source not found at {}\n\
             Run: git submodule update --init linux",
            linux_source.display()
        );
    }

    let output_dir = base_dir.join("output");
    let needs_compile = clean || acornos::rebuild::kernel_needs_compile(&base_dir);
    let needs_install = acornos::rebuild::kernel_needs_install(&base_dir);

    if needs_compile {
        if clean {
            let kernel_build = output_dir.join("kernel-build");
            if kernel_build.exists() {
                println!("Cleaning kernel build directory...");
                std::fs::remove_dir_all(&kernel_build)?;
            }
        }

        println!("Building kernel...");
        let t = Timer::start("Kernel");
        let version = acornos::build::kernel::build_kernel(&linux_source, &output_dir, &base_dir)?;
        acornos::build::kernel::install_kernel(&linux_source, &output_dir, &output_dir.join("staging"))?;
        acornos::rebuild::cache_kernel_hash(&base_dir);
        t.finish();

        println!("\n=== Kernel build complete ===");
        println!("  Version: {}", version);
        println!("  Kernel:  output/staging/boot/vmlinuz");
        println!("  Modules: output/staging/lib/modules/{}/", version);
    } else if needs_install {
        println!("Installing kernel (compile skipped)...");
        let t = Timer::start("Kernel install");
        let version = acornos::build::kernel::install_kernel(&linux_source, &output_dir, &output_dir.join("staging"))?;
        t.finish();

        println!("\n=== Kernel install complete ===");
        println!("  Version: {}", version);
        println!("  Kernel:  output/staging/boot/vmlinuz");
        println!("  Modules: output/staging/lib/modules/{}/", version);
    } else {
        println!("[SKIP] Kernel already built and installed");
        println!("  Use 'build kernel --clean' to force rebuild");
    }

    Ok(())
}

fn cmd_build_squashfs() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if acornos::rebuild::squashfs_needs_rebuild(&base_dir) {
        acornos::artifact::build_squashfs(&base_dir)?;
        acornos::rebuild::cache_squashfs_hash(&base_dir);
    } else {
        println!("[SKIP] Squashfs already built (inputs unchanged)");
        println!("  Delete output/filesystem.squashfs to force rebuild");
    }
    Ok(())
}

fn cmd_initramfs() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
    } else {
        println!("[SKIP] Initramfs already built (inputs unchanged)");
        println!("  Delete output/initramfs-live.cpio.gz to force rebuild");
    }
    Ok(())
}

fn cmd_iso() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Ensure dependencies exist first
    let squashfs = base_dir.join("output/filesystem.squashfs");
    let initramfs = base_dir.join("output/initramfs-live.cpio.gz");

    if !squashfs.exists() {
        println!("Squashfs not found, building...");
        acornos::artifact::build_squashfs(&base_dir)?;
        acornos::rebuild::cache_squashfs_hash(&base_dir);
    }
    if !initramfs.exists() {
        println!("Initramfs not found, building...");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
    }

    if acornos::rebuild::iso_needs_rebuild(&base_dir) {
        acornos::artifact::create_squashfs_iso(&base_dir)?;
    } else {
        println!("[SKIP] ISO already built (components unchanged)");
        println!("  Delete output/acornos.iso to force rebuild");
    }
    Ok(())
}

fn cmd_run() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    acornos::qemu::run_iso(&base_dir, None)
}

fn cmd_test(timeout: u64) -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    acornos::qemu::test_iso(&base_dir, timeout)
}

fn cmd_status() -> Result<()> {
    use acornos::config::AcornConfig;
    use acornos::extract::ExtractPaths;
    use distro_builder::DistroConfig;

    let config = AcornConfig;
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paths = ExtractPaths::new(&base_dir);

    println!("AcornOS Builder Status");
    println!("======================");
    println!();
    println!("Configuration:");
    println!("  OS Name:     {}", config.os_name());
    println!("  OS ID:       {}", config.os_id());
    println!("  ISO Label:   {}", config.iso_label());
    println!("  Init System: {}", config.init_system());
    println!("  Shell:       {}", config.default_shell());
    println!();

    println!("Dependencies (managed by recipe):");
    if paths.iso.exists() {
        println!("  Alpine ISO:      FOUND at {}", paths.iso.display());
    } else {
        println!("  Alpine ISO:      NOT FOUND (run 'recipe resolve deps/alpine.rhai')");
    }

    let apk_static = paths.apk_tools.join("sbin").join("apk.static");
    if apk_static.exists() {
        println!("  apk-tools:       FOUND at {}", apk_static.display());
    } else {
        println!("  apk-tools:       NOT FOUND (run 'recipe resolve deps/alpine.rhai')");
    }

    if paths.rootfs.exists() && paths.rootfs.join("bin").exists() {
        println!("  Rootfs:          CREATED at {}", paths.rootfs.display());
    } else {
        println!("  Rootfs:          NOT CREATED (run 'recipe resolve deps/alpine.rhai')");
    }
    println!();

    // Check Linux kernel source
    let workspace_root = base_dir.parent().expect("AcornOS must be in workspace");
    let linux_source = workspace_root.join("linux");
    println!("Kernel Source:");
    if linux_source.exists() && linux_source.join("Makefile").exists() {
        println!("  Linux source:    FOUND at {}", linux_source.display());
    } else {
        println!("  Linux source:    NOT FOUND (run 'git submodule update --init linux')");
    }
    let kconfig = base_dir.join("kconfig");
    if kconfig.exists() {
        println!("  kconfig:         FOUND at {}", kconfig.display());
    } else {
        println!("  kconfig:         NOT FOUND");
    }

    // Check if we can steal kernel from LevitateOS
    let leviso_bzimage = workspace_root.join("leviso/output/kernel-build/arch/x86/boot/bzImage");
    if leviso_bzimage.exists() {
        println!("  LevitateOS:      KERNEL AVAILABLE (can steal instead of building)");
    }
    println!();

    // Check build artifacts
    let output_dir = base_dir.join("output");
    let kernel = output_dir.join("staging/boot/vmlinuz");
    let kernel_build = output_dir.join("kernel-build");
    let squashfs = output_dir.join("filesystem.squashfs");
    let initramfs = output_dir.join("initramfs-live.cpio.gz");
    let iso = output_dir.join("acornos.iso");

    println!("Build Artifacts:");
    if kernel.exists() {
        let size = std::fs::metadata(&kernel).map(|m| m.len() / 1024 / 1024).unwrap_or(0);
        // Check if kernel-build is a symlink (stolen from leviso)
        let stolen = kernel_build.is_symlink();
        if stolen {
            println!("  Kernel:          STOLEN from LevitateOS ({} MB)", size);
        } else {
            println!("  Kernel:          BUILT ({} MB)", size);
        }
    } else {
        println!("  Kernel:          NOT BUILT");
    }
    if squashfs.exists() {
        let size = std::fs::metadata(&squashfs).map(|m| m.len() / 1024 / 1024).unwrap_or(0);
        println!("  Squashfs:        BUILT ({} MB)", size);
    } else {
        println!("  Squashfs:        NOT BUILT");
    }
    if initramfs.exists() {
        let size = std::fs::metadata(&initramfs).map(|m| m.len() / 1024).unwrap_or(0);
        println!("  Initramfs:       BUILT ({} KB)", size);
    } else {
        println!("  Initramfs:       NOT BUILT");
    }
    if iso.exists() {
        let size = std::fs::metadata(&iso).map(|m| m.len() / 1024 / 1024).unwrap_or(0);
        println!("  ISO:             BUILT ({} MB)", size);
    } else {
        println!("  ISO:             NOT BUILT");
    }
    println!();

    println!("Next steps:");
    if !linux_source.exists() {
        println!("  1. Run 'git submodule update --init linux' to get kernel source");
    } else if !paths.rootfs.exists() {
        println!("  1. Run 'recipe install deps/alpine.rhai' to download and create rootfs");
    } else if !kernel.exists() {
        println!("  1. Run 'acornos build kernel' to build the kernel");
    } else if !squashfs.exists() {
        println!("  1. Run 'acornos build squashfs' to create filesystem.squashfs");
    } else if !initramfs.exists() {
        println!("  1. Run 'acornos initramfs' to create initramfs");
    } else if !iso.exists() {
        println!("  1. Run 'acornos iso' to create bootable ISO");
    } else {
        println!("  ISO ready! Run 'acornos run' to boot in QEMU.");
    }

    Ok(())
}
