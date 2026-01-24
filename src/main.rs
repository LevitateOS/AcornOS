//! AcornOS ISO Builder
//!
//! Builds AcornOS: an Alpine-based daily driver Linux distribution.
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
//!
//! # Status: SKELETON
//!
//! This is a structural skeleton. Commands are not yet implemented.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "acornos")]
#[command(about = "AcornOS ISO builder", long_about = None)]
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
    /// Download Alpine packages
    Download,
    /// Extract Alpine packages to rootfs
    Extract,
    /// Show build status
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

fn cmd_build() -> anyhow::Result<()> {
    unimplemented!("AcornOS build not yet implemented.\n\
        \n\
        This requires:\n\
        - Alpine APK extraction\n\
        - OpenRC service setup\n\
        - Component definitions\n\
        \n\
        See AcornOS/CLAUDE.md for implementation roadmap.")
}

fn cmd_initramfs() -> anyhow::Result<()> {
    unimplemented!("AcornOS initramfs not yet implemented")
}

fn cmd_iso() -> anyhow::Result<()> {
    unimplemented!("AcornOS ISO not yet implemented")
}

fn cmd_run() -> anyhow::Result<()> {
    unimplemented!("AcornOS QEMU runner not yet implemented")
}

fn cmd_download() -> anyhow::Result<()> {
    unimplemented!("Alpine package download not yet implemented.\n\
        \n\
        This requires integration with Alpine APK repositories.")
}

fn cmd_extract() -> anyhow::Result<()> {
    unimplemented!("Alpine package extraction not yet implemented.\n\
        \n\
        This requires apk-tools or equivalent APK extraction.")
}

fn cmd_status() -> anyhow::Result<()> {
    println!("AcornOS Builder Status");
    println!("======================");
    println!();
    println!("Status: SKELETON - Not yet implemented");
    println!();
    println!("AcornOS is a sibling distribution to LevitateOS:");
    println!("  - Alpine Linux base (musl, busybox)");
    println!("  - OpenRC init system");
    println!("  - Daily driver desktop (NOT minimal)");
    println!();
    println!("Next steps:");
    println!("  1. Implement Alpine APK extraction");
    println!("  2. Create OpenRC service components");
    println!("  3. Build initramfs with mdev");
    println!("  4. Create bootable ISO");
    Ok(())
}
