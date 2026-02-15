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
//! # Build EROFS rootfs only
//! acornos build rootfs
//!
//! # Build complete ISO (rootfs + initramfs + ISO)
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

fn open_artifact_store(
    base_dir: &std::path::Path,
) -> Option<distro_builder::artifact_store::ArtifactStore> {
    match distro_builder::artifact_store::ArtifactStore::open_for_distro(base_dir) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("[WARN] Artifact store disabled: {:#}", e);
            None
        }
    }
}

#[derive(Parser)]
#[command(name = "acornos")]
#[command(author, version, about = "AcornOS ISO builder", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download Alpine dependencies (ISO and packages)
    Download {
        #[command(subcommand)]
        what: Option<DownloadTarget>,
    },

    /// Build artifacts (rootfs, or full build)
    Build {
        #[command(subcommand)]
        artifact: Option<BuildArtifact>,
    },

    /// Rebuild only the initramfs
    Initramfs,

    /// Rebuild only the ISO (requires rootfs and initramfs)
    Iso,

    /// Run the ISO in QEMU (GUI)
    Run,

    /// Test the ISO boots correctly (headless, automated)
    Test {
        /// Timeout in seconds (default: 120)
        #[arg(short, long, default_value = "120")]
        timeout: u64,
    },

    /// Validate host tools and prerequisites (xorriso, mkfs.erofs, etc.)
    Preflight,

    /// Show build status and next steps
    Status,
}

#[derive(Subcommand)]
enum DownloadTarget {
    /// Download Alpine Extended ISO and apk-tools
    Alpine,
    /// Download installation tools (recstrap, recfstab, recchroot)
    Tools,
    /// Download everything
    All,
}

#[derive(Subcommand)]
enum BuildArtifact {
    /// Build only the EROFS rootfs image
    Rootfs,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Download { what } => match what {
            Some(DownloadTarget::Alpine) => cmd_download_alpine(),
            Some(DownloadTarget::Tools) => cmd_download_tools(),
            Some(DownloadTarget::All) | None => cmd_download_all(),
        },
        Commands::Build { artifact } => match artifact {
            Some(BuildArtifact::Rootfs) => cmd_build_rootfs(),
            None => cmd_build(),
        },
        Commands::Initramfs => cmd_initramfs(),
        Commands::Iso => cmd_iso(),
        Commands::Run => cmd_run(),
        Commands::Test { timeout } => cmd_test(timeout),
        Commands::Preflight => cmd_preflight(),
        Commands::Status => cmd_status(),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

/// Resolve kernel from existing artifacts or the centralized artifact store.
///
/// Kernel compilation is centralized in `cargo xtask kernels build acorn` (nightly policy).
/// This distro builder should never compile kernels implicitly.
fn resolve_kernel(base_dir: &std::path::Path) -> Result<()> {
    let store = open_artifact_store(base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(base_dir);
    let staging = output_dir.join("staging");
    let vmlinuz = staging.join("boot/vmlinuz");
    if vmlinuz.exists() {
        println!("[SKIP] Kernel already built and installed");
        return Ok(());
    }

    // Try to restore from the centralized artifact store first (no compilation).
    if let Some(store) = &store {
        let key = output_dir.join(".kernel-inputs.hash");
        match distro_builder::artifact_store::try_restore_kernel_payload_from_key(
            store, &key, &staging,
        ) {
            Ok(true) => {
                println!("[RESTORE] Kernel payload restored from artifact store");
                return Ok(());
            }
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore kernel payload from artifact store: {:#}",
                e
            ),
        }
    }

    anyhow::bail!(
        "No kernel available.\n\n\
         Kernel compilation is centralized in xtask (nightly build-hours policy).\n\
         Build the kernels first, then re-run this command:\n\
           cargo xtask kernels build acorn"
    )
}

fn cmd_build() -> Result<()> {
    use distro_builder::timing::Timer;
    use std::time::Instant;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let store = open_artifact_store(&base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);
    let build_start = Instant::now();

    println!("=== Full AcornOS Build ===\n");

    // 1. Resolve kernel (must already be built via xtask)
    resolve_kernel(&base_dir)?;

    // Try to restore build outputs from the centralized artifact store if the
    // output files are missing but input hashes are known.
    if let Some(store) = &store {
        let rootfs_key = output_dir.join(".rootfs-inputs.hash");
        let rootfs_out = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "rootfs_erofs",
            &rootfs_key,
            &rootfs_out,
        ) {
            Ok(true) => println!("\n[RESTORE] Rootfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore rootfs from artifact store: {:#}",
                e
            ),
        }

        let initramfs_key = output_dir.join(".initramfs-inputs.hash");
        let initramfs_out = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "initramfs",
            &initramfs_key,
            &initramfs_out,
        ) {
            Ok(true) => println!("\n[RESTORE] Initramfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore initramfs from artifact store: {:#}",
                e
            ),
        }
    }

    // 2. Build EROFS rootfs (skip if inputs unchanged)
    if acornos::rebuild::rootfs_needs_rebuild(&base_dir) {
        println!("\nBuilding EROFS system image...");
        let t = Timer::start("EROFS");
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
        if let Some(store) = &store {
            let key = output_dir.join(".rootfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!("[WARN] Failed to store rootfs in artifact store: {:#}", e);
            }
        }
        t.finish();
    } else {
        println!("\n[SKIP] EROFS rootfs already built (inputs unchanged)");
    }

    // 3. Build initramfs (skip if inputs unchanged)
    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        println!("\nBuilding tiny initramfs...");
        let t = Timer::start("Initramfs");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
        if let Some(store) = &store {
            let key = output_dir.join(".initramfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "initramfs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store initramfs in artifact store: {:#}",
                    e
                );
            }
        }
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

    // 4. Build ISO (skip if components unchanged)
    if acornos::rebuild::iso_needs_rebuild(&base_dir) {
        println!("\nBuilding ISO...");
        let t = Timer::start("ISO");
        acornos::artifact::create_iso(&base_dir)?;
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
    println!(
        "  ISO: {}",
        output_dir.join(distro_spec::acorn::ISO_FILENAME).display()
    );
    println!("\nNext: acornos run");

    Ok(())
}

fn cmd_build_rootfs() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let store = open_artifact_store(&base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);

    if let Some(store) = &store {
        let key = output_dir.join(".rootfs-inputs.hash");
        let out = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "rootfs_erofs",
            &key,
            &out,
        ) {
            Ok(true) => println!("[RESTORE] Rootfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore rootfs from artifact store: {:#}",
                e
            ),
        }
    }

    if acornos::rebuild::rootfs_needs_rebuild(&base_dir) {
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
        if let Some(store) = &store {
            let key = output_dir.join(".rootfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!("[WARN] Failed to store rootfs in artifact store: {:#}", e);
            }
        }
    } else {
        println!("[SKIP] EROFS rootfs already built (inputs unchanged)");
        println!(
            "  Delete {} to force rebuild",
            output_dir.join(distro_spec::acorn::ROOTFS_NAME).display()
        );
    }
    Ok(())
}

fn cmd_initramfs() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let store = open_artifact_store(&base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);

    if let Some(store) = &store {
        let key = output_dir.join(".initramfs-inputs.hash");
        let out = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);
        match distro_builder::artifact_store::try_restore_file_from_key(
            store,
            "initramfs",
            &key,
            &out,
        ) {
            Ok(true) => println!("[RESTORE] Initramfs restored from artifact store"),
            Ok(false) => {}
            Err(e) => eprintln!(
                "[WARN] Failed to restore initramfs from artifact store: {:#}",
                e
            ),
        }
    }

    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
        if let Some(store) = &store {
            let key = output_dir.join(".initramfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);
            if let Err(e) = distro_builder::artifact_store::try_store_file_from_key(
                store,
                "initramfs",
                &key,
                &out,
                std::collections::BTreeMap::new(),
            ) {
                eprintln!(
                    "[WARN] Failed to store initramfs in artifact store: {:#}",
                    e
                );
            }
        }
    } else {
        println!("[SKIP] Initramfs already built (inputs unchanged)");
        println!(
            "  Delete {} to force rebuild",
            output_dir
                .join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT)
                .display()
        );
    }
    Ok(())
}

fn cmd_iso() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let store = open_artifact_store(&base_dir);
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);

    // Ensure dependencies exist first
    let rootfs = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
    let initramfs = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);

    if !rootfs.exists() {
        if let Some(store) = &store {
            let key = output_dir.join(".rootfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::ROOTFS_NAME);
            match distro_builder::artifact_store::try_restore_file_from_key(
                store,
                "rootfs_erofs",
                &key,
                &out,
            ) {
                Ok(true) => println!("EROFS rootfs restored from artifact store."),
                Ok(false) => {}
                Err(e) => eprintln!(
                    "[WARN] Failed to restore rootfs from artifact store: {:#}",
                    e
                ),
            }
        }
        if !rootfs.exists() {
            println!("EROFS rootfs not found, building...");
            acornos::artifact::build_rootfs(&base_dir)?;
            acornos::rebuild::cache_rootfs_hash(&base_dir);
        }
    }
    if !initramfs.exists() {
        if let Some(store) = &store {
            let key = output_dir.join(".initramfs-inputs.hash");
            let out = output_dir.join(distro_spec::acorn::INITRAMFS_LIVE_OUTPUT);
            match distro_builder::artifact_store::try_restore_file_from_key(
                store,
                "initramfs",
                &key,
                &out,
            ) {
                Ok(true) => println!("Initramfs restored from artifact store."),
                Ok(false) => {}
                Err(e) => eprintln!(
                    "[WARN] Failed to restore initramfs from artifact store: {:#}",
                    e
                ),
            }
        }
        if !initramfs.exists() {
            println!("Initramfs not found, building...");
            acornos::artifact::build_tiny_initramfs(&base_dir)?;
            acornos::rebuild::cache_initramfs_hash(&base_dir);
        }
    }

    if acornos::rebuild::iso_needs_rebuild(&base_dir) {
        acornos::artifact::create_iso(&base_dir)?;
    } else {
        println!("[SKIP] ISO already built (components unchanged)");
        println!(
            "  Delete {} to force rebuild",
            output_dir.join(distro_spec::acorn::ISO_FILENAME).display()
        );
    }
    Ok(())
}

fn cmd_run() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    acornos::qemu::run_iso(&base_dir, None)
}

fn cmd_test(timeout: u64) -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);
    let iso_path = output_dir.join(distro_spec::acorn::ISO_FILENAME);
    distro_builder::qemu::test_iso_boot(
        &iso_path,
        timeout,
        "acorn",
        "00-acorn-test.sh",
        distro_spec::acorn::QEMU_CPU_MODE,
        distro_spec::acorn::QEMU_MEMORY_GB,
    )
}

fn cmd_preflight() -> Result<()> {
    use acornos::preflight::PreflightChecker;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let checker = PreflightChecker::new(&base_dir);

    // Run preflight checks (this is async)
    let rt = tokio::runtime::Runtime::new()?;
    let report = rt.block_on(async { checker.run_all().await });

    report.print_summary();

    if !report.is_ok() {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_download_all() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    println!("Resolving all dependencies...\n");

    // Alpine ISO and packages
    let alpine = distro_builder::recipe::alpine::alpine(&base_dir)?;
    distro_builder::alpine::keys::install_keys(
        &alpine.rootfs,
        distro_spec::acorn::packages::ALPINE_KEYS,
    )?;
    println!("Alpine:  {} [OK]", alpine.iso.display());

    // Installation tools
    distro_builder::recipe::install_tools(&base_dir)?;

    println!("\nAll dependencies resolved.");
    Ok(())
}

fn cmd_download_alpine() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let alpine = distro_builder::recipe::alpine::alpine(&base_dir)?;
    distro_builder::alpine::keys::install_keys(
        &alpine.rootfs,
        distro_spec::acorn::packages::ALPINE_KEYS,
    )?;

    println!("Alpine ISO and packages:");
    println!("  ISO:         {}", alpine.iso.display());
    println!("  rootfs:      {}", alpine.rootfs.display());

    // Install Tier 0-2 packages (dependencies for rootfs build)
    println!("\nInstalling Tier 0-2 packages...");
    distro_builder::recipe::packages(&base_dir)?;
    println!("✓ Packages installed");

    Ok(())
}

fn cmd_download_tools() -> Result<()> {
    use distro_spec::shared::LEVITATE_CARGO_TOOLS;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);

    println!("Installing tools via recipes...\n");
    distro_builder::recipe::install_tools(&base_dir)?;

    // Show what was installed
    let staging_bin = output_dir.join("staging/usr/bin");
    println!("\nTools installed:");
    for tool in LEVITATE_CARGO_TOOLS {
        let path = staging_bin.join(tool);
        let status = if path.exists() { "OK" } else { "MISSING" };
        println!(
            "  {:10} {} [{}]",
            format!("{}:", tool),
            path.display(),
            status
        );
    }

    Ok(())
}

fn cmd_status() -> Result<()> {
    use acornos::config::AcornConfig;
    use distro_builder::alpine::extract::ExtractPaths;
    use distro_contract::DistroConfig;

    let config = AcornConfig;
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = distro_builder::artifact_store::central_output_dir_for_distro(&base_dir);
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
        println!("  Alpine ISO:      NOT FOUND (run 'acornos download alpine')");
    }

    let apk_static = paths.apk_tools.join("sbin").join("apk.static");
    if apk_static.exists() {
        println!("  apk-tools:       FOUND at {}", apk_static.display());
    } else {
        println!("  apk-tools:       NOT FOUND (run 'acornos download alpine')");
    }

    if paths.rootfs.exists() && paths.rootfs.join("bin").exists() {
        println!("  Rootfs:          CREATED at {}", paths.rootfs.display());
    } else {
        println!("  Rootfs:          NOT CREATED (run 'acornos download alpine')");
    }
    println!();

    // Check Linux kernel source
    let kernel_spec = &distro_spec::acorn::KERNEL_SOURCE;
    let tarball_source = base_dir
        .join("downloads")
        .join(kernel_spec.source_dir_name());
    println!("Kernel Source (v{}):", kernel_spec.version);
    if tarball_source.join("Makefile").exists() {
        println!("  Linux source:    FOUND at {}", tarball_source.display());
    } else {
        println!("  Linux source:    NOT DOWNLOADED (will fetch from cdn.kernel.org)");
    }
    let kconfig = base_dir.join("kconfig");
    if kconfig.exists() {
        println!("  kconfig:         FOUND at {}", kconfig.display());
    } else {
        println!("  kconfig:         NOT FOUND");
    }

    // Check build artifacts
    let kernel = output_dir.join("staging/boot/vmlinuz");
    let rootfs = output_dir.join("filesystem.erofs");
    let initramfs = output_dir.join("initramfs-live.cpio.gz");
    let iso = output_dir.join(distro_spec::acorn::ISO_FILENAME);

    println!("Build Artifacts:");
    if kernel.exists() {
        let size = std::fs::metadata(&kernel)
            .map(|m| m.len() / 1024 / 1024)
            .unwrap_or(0);

        // Prefer provenance from the kernel release (modules dir name), since
        // output/kernel-build may be missing even when a kernel is present.
        let kernel_release = {
            let staging = output_dir.join("staging");
            let candidates = [staging.join("lib/modules"), staging.join("usr/lib/modules")];
            let mut found = None;
            for dir in candidates {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                found = Some(name.to_string());
                                break;
                            }
                        }
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            found
        };

        let expected_suffix = kernel_spec.localversion;
        let built_for_distro = kernel_release
            .as_deref()
            .map(|r| r.contains(expected_suffix))
            .unwrap_or(false);

        let release_suffix = kernel_release
            .as_deref()
            .map(|r| format!(" ({})", r))
            .unwrap_or_default();

        if built_for_distro {
            println!("  Kernel:          PRESENT ({} MB){}", size, release_suffix);
        } else {
            println!("  Kernel:          PRESENT ({} MB){}", size, release_suffix);
            println!(
                "                  WARNING: expected suffix '{}' (build via: cargo xtask kernels build acorn)",
                expected_suffix
            );
        }
    } else {
        println!("  Kernel:          NOT BUILT");
    }
    if rootfs.exists() {
        let size = std::fs::metadata(&rootfs)
            .map(|m| m.len() / 1024 / 1024)
            .unwrap_or(0);
        println!("  EROFS:           BUILT ({} MB)", size);
    } else {
        println!("  EROFS:           NOT BUILT");
    }
    if initramfs.exists() {
        let size = std::fs::metadata(&initramfs)
            .map(|m| m.len() / 1024)
            .unwrap_or(0);
        println!("  Initramfs:       BUILT ({} KB)", size);
    } else {
        println!("  Initramfs:       NOT BUILT");
    }
    if iso.exists() {
        let size = std::fs::metadata(&iso)
            .map(|m| m.len() / 1024 / 1024)
            .unwrap_or(0);
        println!("  ISO:             BUILT ({} MB)", size);
    } else {
        println!("  ISO:             NOT BUILT");
    }
    println!();

    println!("Next steps:");
    if !paths.rootfs.exists() {
        println!("  1. Run 'acornos download alpine' to download and create rootfs");
    } else if !kernel.exists() {
        println!("  1. Run 'cargo xtask kernels build acorn' to build the kernel");
    } else if !rootfs.exists() {
        println!("  1. Run 'acornos build rootfs' to create filesystem.erofs");
    } else if !initramfs.exists() {
        println!("  1. Run 'acornos initramfs' to create initramfs");
    } else if !iso.exists() {
        println!("  1. Run 'acornos iso' to create bootable ISO");
    } else {
        println!("  ISO ready! Run 'acornos run' to boot in QEMU.");
    }

    Ok(())
}
