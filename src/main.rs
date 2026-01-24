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
//! # Build complete ISO (not yet implemented)
//! acornos build
//!
//! # Run in QEMU (not yet implemented)
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
    /// Build the complete ISO (squashfs + initramfs + ISO)
    Build,

    /// Rebuild only the initramfs
    Initramfs,

    /// Rebuild only the ISO (requires squashfs and initramfs)
    Iso,

    /// Run the ISO in QEMU
    Run,

    /// Download Alpine Extended ISO and apk-tools
    Download,

    /// Extract Alpine ISO and create rootfs
    Extract,

    /// Show build status and next steps
    Status,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build => cmd_build(),
        Commands::Initramfs => cmd_initramfs(),
        Commands::Iso => cmd_iso(),
        Commands::Run => cmd_run(),
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
    unimplemented!(
        "AcornOS build not yet implemented.\n\
        \n\
        This requires:\n\
        - Alpine APK extraction (DONE - use 'acornos extract')\n\
        - OpenRC service setup\n\
        - Component definitions\n\
        \n\
        See AcornOS/CLAUDE.md for implementation roadmap."
    )
}

fn cmd_initramfs() -> Result<()> {
    unimplemented!(
        "AcornOS initramfs not yet implemented.\n\
        \n\
        This requires:\n\
        - busybox from Alpine\n\
        - mdev or eudev setup\n\
        - OpenRC-compatible init script"
    )
}

fn cmd_iso() -> Result<()> {
    unimplemented!(
        "AcornOS ISO not yet implemented.\n\
        \n\
        This requires:\n\
        - Built squashfs and initramfs\n\
        - GRUB configuration\n\
        - xorriso packaging"
    )
}

fn cmd_run() -> Result<()> {
    unimplemented!(
        "AcornOS QEMU runner not yet implemented.\n\
        \n\
        This requires:\n\
        - Built ISO\n\
        - QEMU with UEFI support"
    )
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

    println!("Next steps:");
    if !paths.iso.exists() {
        println!("  1. Run 'acornos download' to download Alpine Extended ISO");
    } else if !paths.rootfs.exists() {
        println!("  1. Run 'acornos extract' to extract ISO and create rootfs");
    } else {
        println!("  1. Rootfs ready! Build/initramfs/ISO commands not yet implemented.");
    }

    Ok(())
}
