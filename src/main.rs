//! AcornOS ISO Builder CLI
//!
//! Builds AcornOS: an Alpine-based daily driver Linux distribution.
//!
//! # Usage
//!
//! ```bash
//! # Show current status
//! acornos status
//!
//! # Download Alpine Extended ISO (~1GB)
//! acornos download
//!
//! # Extract ISO and create rootfs
//! acornos extract
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

mod extract;

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

    /// Download Alpine Extended ISO and apk-tools
    Download,

    /// Extract Alpine ISO and create rootfs
    Extract,

    /// Show build status and next steps
    Status,
}

#[derive(Subcommand)]
enum BuildArtifact {
    /// Build only the squashfs from rootfs
    Squashfs,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { artifact } => match artifact {
            Some(BuildArtifact::Squashfs) => cmd_build_squashfs(),
            None => cmd_build(),
        },
        Commands::Initramfs => cmd_initramfs(),
        Commands::Iso => cmd_iso(),
        Commands::Run => cmd_run(),
        Commands::Test { timeout } => cmd_test(timeout),
        Commands::Download => cmd_download(),
        Commands::Extract => cmd_extract(),
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

    // Full build: squashfs + initramfs + ISO
    // Skips anything already built, rebuilds only on changes.
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let build_start = Instant::now();

    println!("=== Full AcornOS Build ===\n");

    // 1. Build squashfs (skip if inputs unchanged)
    if acornos::rebuild::squashfs_needs_rebuild(&base_dir) {
        println!("Building squashfs system image...");
        let t = Timer::start("Squashfs");
        acornos::artifact::build_squashfs(&base_dir)?;
        acornos::rebuild::cache_squashfs_hash(&base_dir);
        t.finish();
    } else {
        println!("[SKIP] Squashfs already built (inputs unchanged)");
    }

    // 2. Build initramfs (skip if inputs unchanged)
    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        println!("\nBuilding tiny initramfs...");
        let t = Timer::start("Initramfs");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 3. Build ISO (skip if components unchanged)
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

fn cmd_download() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Create tokio runtime for async download
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(extract::cmd_download_impl(&base_dir))
}

fn cmd_extract() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    extract::cmd_extract_impl(&base_dir)
}

fn cmd_status() -> Result<()> {
    use acornos::config::AcornConfig;
    use distro_builder::DistroConfig;

    let config = AcornConfig;
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paths = extract::ExtractPaths::new(&base_dir);

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

    println!("Downloads:");
    if paths.iso.exists() {
        println!("  Alpine ISO:      FOUND at {}", paths.iso.display());
    } else {
        println!("  Alpine ISO:      NOT FOUND (run 'acornos download')");
    }

    let apk_static = paths.apk_tools.join("sbin").join("apk.static");
    if apk_static.exists() {
        println!("  apk-tools:       FOUND at {}", apk_static.display());
    } else {
        println!("  apk-tools:       NOT FOUND (run 'acornos download')");
    }
    println!();

    println!("Extraction:");
    if paths.iso_contents.exists() && paths.iso_contents.join("apks").exists() {
        println!("  ISO contents:    EXTRACTED at {}", paths.iso_contents.display());
    } else {
        println!("  ISO contents:    NOT EXTRACTED (run 'acornos extract')");
    }

    if paths.rootfs.exists() && paths.rootfs.join("bin").exists() {
        println!("  Rootfs:          CREATED at {}", paths.rootfs.display());
    } else {
        println!("  Rootfs:          NOT CREATED (run 'acornos extract')");
    }
    println!();

    // Check build artifacts
    let output_dir = base_dir.join("output");
    let squashfs = output_dir.join("filesystem.squashfs");
    let initramfs = output_dir.join("initramfs-live.cpio.gz");
    let iso = output_dir.join("acornos.iso");

    println!("Build Artifacts:");
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
    if !paths.iso.exists() {
        println!("  1. Run 'acornos download' to download Alpine Extended ISO");
    } else if !paths.rootfs.exists() {
        println!("  1. Run 'acornos extract' to extract ISO and create rootfs");
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
