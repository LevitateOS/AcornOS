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

        /// Build the kernel from source (~1 hour). Requires --dangerously-waste-the-users-time.
        #[arg(long)]
        kernel: bool,

        /// Confirm that you really want to spend ~1 hour building the kernel.
        #[arg(long)]
        dangerously_waste_the_users_time: bool,
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
    /// Download Linux kernel source
    Linux,
    /// Download installation tools (recstrap, recfstab, recchroot)
    Tools,
    /// Download everything
    All,
}

#[derive(Subcommand)]
enum BuildArtifact {
    /// Build the kernel from source
    Kernel {
        /// Clean build directory before building
        #[arg(long)]
        clean: bool,
    },
    /// Build only the EROFS rootfs image
    Rootfs,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Download { what } => match what {
            Some(DownloadTarget::Alpine) => cmd_download_alpine(),
            Some(DownloadTarget::Linux) => cmd_download_linux(),
            Some(DownloadTarget::Tools) => cmd_download_tools(),
            Some(DownloadTarget::All) | None => cmd_download_all(),
        },
        Commands::Build { artifact, kernel, dangerously_waste_the_users_time } => {
            use distro_contract::kernel::{KernelBuildGuard, KernelGuard};
            match artifact {
                Some(BuildArtifact::Kernel { clean }) => {
                    KernelGuard::new(true, dangerously_waste_the_users_time,
                        "cargo run -- build kernel --dangerously-waste-the-users-time",
                    ).require_kernel_confirmation();
                    cmd_build_kernel(clean)
                }
                Some(BuildArtifact::Rootfs) => cmd_build_rootfs(),
                None => {
                    if kernel {
                        KernelGuard::new(true, dangerously_waste_the_users_time,
                            "cargo run -- build --kernel --dangerously-waste-the-users-time",
                        ).require_kernel_confirmation();
                        cmd_build_with_kernel()
                    } else {
                        cmd_build()
                    }
                }
            }
        }
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

/// Resolve kernel via recipe: handles download, theft from leviso, build, and install.
/// Returns Ok if kernel is available in staging after this call.
fn resolve_kernel(base_dir: &std::path::Path) -> Result<()> {
    let vmlinuz = base_dir.join("output/staging/boot/vmlinuz");
    if vmlinuz.exists() {
        println!("[SKIP] Kernel already built and installed");
        return Ok(());
    }

    // Run the recipe — it handles theft from leviso internally
    println!("Resolving kernel via recipe...");
    let linux = distro_builder::recipe::linux::linux(base_dir, &distro_spec::acorn::KERNEL_SOURCE)?;

    if !linux.vmlinuz.exists() {
        anyhow::bail!(
            "No kernel available!\n\n\
             Options:\n\
             1. Build LevitateOS first (leviso build) — AcornOS can steal its kernel\n\
             2. Build AcornOS kernel:  cargo run -- build --kernel --dangerously-waste-the-users-time\n\
             3. Build kernel only:     cargo run -- build kernel --dangerously-waste-the-users-time\n\n\
             Kernel source will be downloaded automatically from cdn.kernel.org (v{}).",
            distro_spec::acorn::KERNEL_SOURCE.version
        );
    }

    Ok(())
}

fn cmd_build() -> Result<()> {
    use distro_builder::alpine::timing::Timer;
    use std::time::Instant;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let build_start = Instant::now();

    println!("=== Full AcornOS Build ===\n");

    // 1. Resolve kernel (use existing or steal — never build without explicit flag)
    resolve_kernel(&base_dir)?;

    // 2. Build EROFS rootfs (skip if inputs unchanged)
    if acornos::rebuild::rootfs_needs_rebuild(&base_dir) {
        println!("\nBuilding EROFS system image...");
        let t = Timer::start("EROFS");
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
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
    println!("  ISO: output/acornos.iso");
    println!("\nNext: acornos run");

    Ok(())
}

fn cmd_build_with_kernel() -> Result<()> {
    use distro_builder::alpine::timing::Timer;
    use std::time::Instant;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let build_start = Instant::now();

    println!("=== Full AcornOS Build (with kernel) ===\n");

    // Build kernel via recipe (handles download + build + install)
    let needs_compile = acornos::rebuild::kernel_needs_compile(&base_dir);
    if needs_compile {
        println!("Building kernel from source (~1 hour)...");
        let t = Timer::start("Kernel");
        distro_builder::recipe::linux::linux(&base_dir, &distro_spec::acorn::KERNEL_SOURCE)?;
        acornos::rebuild::cache_kernel_hash(&base_dir);
        t.finish();
    } else {
        println!("[SKIP] Kernel already built and installed");
    }

    // Continue with rest of build
    if acornos::rebuild::rootfs_needs_rebuild(&base_dir) {
        println!("\nBuilding EROFS system image...");
        let t = Timer::start("EROFS");
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] EROFS rootfs already built (inputs unchanged)");
    }

    if acornos::rebuild::initramfs_needs_rebuild(&base_dir) {
        println!("\nBuilding tiny initramfs...");
        let t = Timer::start("Initramfs");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
        t.finish();
    } else {
        println!("\n[SKIP] Initramfs already built (inputs unchanged)");
    }

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
    println!("  ISO: output/acornos.iso");
    println!("\nNext: acornos run");

    Ok(())
}

fn cmd_build_kernel(clean: bool) -> Result<()> {
    use distro_builder::alpine::timing::Timer;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if clean {
        let kernel_build = base_dir.join("output/kernel-build");
        if kernel_build.exists() {
            println!("Cleaning kernel build directory...");
            std::fs::remove_dir_all(&kernel_build)?;
        }
    }

    let needs_compile = clean || acornos::rebuild::kernel_needs_compile(&base_dir);
    let needs_install = acornos::rebuild::kernel_needs_install(&base_dir);

    if needs_compile || needs_install {
        println!("Building kernel via recipe...");
        let t = Timer::start("Kernel");
        let linux = distro_builder::recipe::linux::linux(&base_dir, &distro_spec::acorn::KERNEL_SOURCE)?;
        acornos::rebuild::cache_kernel_hash(&base_dir);
        t.finish();

        println!("\n=== Kernel build complete ===");
        println!("  Version: {}", linux.version);
        println!("  Kernel:  output/staging/boot/vmlinuz");
    } else {
        println!("[SKIP] Kernel already built and installed");
        println!("  Use 'build kernel --clean' to force rebuild");
    }

    Ok(())
}

fn cmd_build_rootfs() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    if acornos::rebuild::rootfs_needs_rebuild(&base_dir) {
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
    } else {
        println!("[SKIP] EROFS rootfs already built (inputs unchanged)");
        println!("  Delete output/filesystem.erofs to force rebuild");
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
    let rootfs = base_dir.join("output/filesystem.erofs");
    let initramfs = base_dir.join("output/initramfs-live.cpio.gz");

    if !rootfs.exists() {
        println!("EROFS rootfs not found, building...");
        acornos::artifact::build_rootfs(&base_dir)?;
        acornos::rebuild::cache_rootfs_hash(&base_dir);
    }
    if !initramfs.exists() {
        println!("Initramfs not found, building...");
        acornos::artifact::build_tiny_initramfs(&base_dir)?;
        acornos::rebuild::cache_initramfs_hash(&base_dir);
    }

    if acornos::rebuild::iso_needs_rebuild(&base_dir) {
        acornos::artifact::create_iso(&base_dir)?;
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
    let iso_path = base_dir
        .join("output")
        .join(distro_spec::acorn::ISO_FILENAME);
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

    // Linux kernel (via recipe)
    let linux = distro_builder::recipe::linux::linux(&base_dir, &distro_spec::acorn::KERNEL_SOURCE)?;
    println!("Linux:   {} [OK]", linux.source.display());

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

fn cmd_download_linux() -> Result<()> {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = &distro_spec::acorn::KERNEL_SOURCE;

    println!("Linux kernel (AcornOS target: {} mainline):", source.version);

    let linux = distro_builder::recipe::linux::linux(&base_dir, source)?;
    println!("  Source:      {}", linux.source.display());

    if linux.vmlinuz.exists() {
        println!("  Installed:   {}", linux.vmlinuz.display());
    } else {
        println!("  Kernel:      NOT INSTALLED YET");
    }

    Ok(())
}

fn cmd_download_tools() -> Result<()> {
    use distro_spec::shared::LEVITATE_CARGO_TOOLS;

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    println!("Installing tools via recipes...\n");
    distro_builder::recipe::install_tools(&base_dir)?;

    // Show what was installed
    let staging_bin = base_dir.join("output/staging/usr/bin");
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
    let tarball_source = base_dir.join("downloads").join(kernel_spec.source_dir_name());
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

    // Check if we can steal kernel from LevitateOS
    let workspace_root = base_dir.parent().expect("AcornOS must be in workspace");
    let leviso_bzimage = workspace_root.join("leviso/output/kernel-build/arch/x86/boot/bzImage");
    if leviso_bzimage.exists() {
        println!("  LevitateOS:      KERNEL AVAILABLE (can steal instead of building)");
    }
    println!();

    // Check build artifacts
    let output_dir = base_dir.join("output");
    let kernel = output_dir.join("staging/boot/vmlinuz");
    let kernel_build = output_dir.join("kernel-build");
    let rootfs = output_dir.join("filesystem.erofs");
    let initramfs = output_dir.join("initramfs-live.cpio.gz");
    let iso = output_dir.join("acornos.iso");

    println!("Build Artifacts:");
    if kernel.exists() {
        let size = std::fs::metadata(&kernel)
            .map(|m| m.len() / 1024 / 1024)
            .unwrap_or(0);
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
        println!("  1. Run 'acornos build kernel' to build the kernel");
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
