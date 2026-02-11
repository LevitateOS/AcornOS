// Supplementary APK Installation - dependency recipe
//
// Provides: Additional packages beyond alpine-base
// Depends on: alpine.rhai (must run first)
// Output: Packages installed in BUILD_DIR/rootfs
//
// This separates package installation from base rootfs creation
// for faster iteration when changing the package list.
//
// === Globals (provided by executor) ===
//
// BUILD_DIR   - Build directory (where alpine.rhai outputs are)
// RECIPE_DIR  - Directory containing this recipe file

// Package tiers - these build on top of alpine-base from alpine.rhai
//
// Tier 1: Core System - hardware support, device management
// Tier 2: Daily Driver - network, utilities, services
// Tier 3: Live ISO Tools - installation utilities

let TIER1_CORE = [
    // Device management
    "eudev",
    "eudev-openrc",
    // Firmware
    "linux-firmware",
    "intel-ucode",
    "amd-ucode",
    "sof-firmware",
    // Storage
    "cryptsetup",
    "lvm2",
    "btrfs-progs",
    "device-mapper",
    // Shell & coreutils
    "util-linux-login",
    "bash",
    "coreutils",
    "doas",
    "grep",
    "sed",
    "gawk",
    "findutils",
];

let TIER2_DAILY = [
    // Network
    "ifupdown-ng",
    "dhcpcd",
    "iproute2",
    "iputils",
    "iwd",
    "wireless-regdb",
    "ca-certificates",
    // Time
    "tzdata",
    "chrony",
    // Utilities
    "curl",
    "less",
    "vim",
    "htop",
    // Hardware tools
    "pciutils",
    "usbutils",
    "dmidecode",
    "ethtool",
    "smartmontools",
    "hdparm",
    "nvme-cli",
    // SSH
    "openssh",
    "openssh-server-common",
];

let TIER3_LIVE = [
    // Partitioning
    "parted",
    "sgdisk",
    // Filesystems
    "xfsprogs",
    "squashfs-tools",
];

// Version for cache invalidation - bump when package list changes
let PACKAGE_LIST_VERSION = "1.0.2";

let ctx = #{
    description: "Supplementary APKs for Alpine rootfs",
    name: "packages",
    version: "1.0.2",
    rootfs_path: "",
    apk_static_path: "",
    packages_installed: 0,
};

// === ACQUIRE ===
// Verify alpine.rhai outputs exist

fn is_acquired(ctx) {
    let rootfs = join_path(BUILD_DIR, "rootfs");
    let apk_static = join_path(BUILD_DIR, "apk-tools/sbin/apk.static");

    if !is_dir(join_path(rootfs, "usr")) {
        throw "rootfs not found - run alpine.rhai first";
    }
    if !is_file(apk_static) {
        throw "apk.static not found - run alpine.rhai first";
    }

    ctx.rootfs_path = rootfs;
    ctx.apk_static_path = apk_static;
    ctx
}

fn acquire(ctx) {
    // Nothing to acquire - we depend on alpine.rhai outputs
    is_acquired(ctx)
}

// === BUILD ===
// Install packages using apk

fn is_built(ctx) {
    let version_file = join_path(BUILD_DIR, ".packages-version");
    let manifest_file = join_path(BUILD_DIR, ".packages-manifest");

    // Check version matches
    let actual_version = if is_file(version_file) { trim(read_file(version_file)) } else { "" };
    if actual_version != ctx.version {
        throw "package list version changed - rebuild required";
    }

    // Check manifest exists
    if !is_file(manifest_file) {
        throw "manifest missing - rebuild required";
    }

    ctx
}

fn build(ctx) {
    let rootfs = join_path(BUILD_DIR, "rootfs");
    let apk_static = join_path(BUILD_DIR, "apk-tools/sbin/apk.static");
    let version_file = join_path(BUILD_DIR, ".packages-version");
    let manifest_file = join_path(BUILD_DIR, ".packages-manifest");

    // Check if already at correct version
    let actual_version = if is_file(version_file) { trim(read_file(version_file)) } else { "" };
    if actual_version == ctx.version && is_file(manifest_file) {
        log("Packages already installed (version " + ctx.version + ")");
        return ctx;
    }

    log("Installing supplementary packages...");

    // Initialize APK database if missing
    let apk_db_dir = join_path(rootfs, "var/lib/apk");
    if !is_dir(apk_db_dir) {
        mkdir(apk_db_dir);
        log("Initialized APK database directory");
    }

    let apk_cmd = apk_static + " --root " + rootfs + " --no-progress --allow-untrusted add ";

    // === Tier 1: Core System ===
    log("Installing Tier 1: Core system...");
    let tier1 = "eudev eudev-openrc linux-firmware intel-ucode amd-ucode sof-firmware";
    shell(apk_cmd + tier1);

    let tier1b = "cryptsetup lvm2 btrfs-progs device-mapper";
    shell(apk_cmd + tier1b);

    let tier1c = "util-linux-login bash coreutils doas grep sed gawk findutils";
    shell(apk_cmd + tier1c);

    // === Tier 2: Daily Driver ===
    log("Installing Tier 2: Daily driver...");
    let tier2a = "ifupdown-ng dhcpcd iproute2 iputils iwd wireless-regdb ca-certificates tzdata chrony";
    shell(apk_cmd + tier2a);

    let tier2b = "curl less vim htop pciutils usbutils dmidecode ethtool";
    shell(apk_cmd + tier2b);

    let tier2c = "smartmontools hdparm nvme-cli openssh";
    shell(apk_cmd + tier2c);

    // === Tier 3: Live ISO Tools ===
    log("Installing Tier 3: Live ISO tools...");
    let tier3 = "parted sgdisk xfsprogs squashfs-tools";
    shell(apk_cmd + tier3);

    // Write version and manifest
    write_file(version_file, ctx.version);

    // Create manifest of installed packages
    let manifest = shell_output(apk_static + " --root " + rootfs + " info 2>/dev/null | wc -l");
    write_file(manifest_file, trim(manifest));

    ctx.packages_installed = parse_int(trim(manifest));
    log("Package installation complete (" + ctx.packages_installed + " packages total)");

    ctx
}

// === INSTALL ===
// Verify key binaries exist

fn is_installed(ctx) {
    let rootfs = join_path(BUILD_DIR, "rootfs");
    let install_marker = join_path(BUILD_DIR, ".packages-installed");

    if !is_file(install_marker) {
        throw "install marker missing";
    }

    // Verify key binaries
    let key_binaries = [
        "usr/bin/bash",
        "usr/bin/vim",
        "usr/bin/curl",
        "usr/sbin/parted",
        "usr/bin/htop",
    ];

    for bin in key_binaries {
        if !is_file(join_path(rootfs, bin)) {
            throw "key binary missing: " + bin;
        }
    }

    ctx
}

fn install(ctx) {
    let rootfs = join_path(BUILD_DIR, "rootfs");
    let install_marker = join_path(BUILD_DIR, ".packages-installed");

    log("Verifying key binaries...");

    let key_binaries = [
        ["usr/bin/bash", "bash"],
        ["usr/bin/vim", "vim"],
        ["usr/bin/curl", "curl"],
        ["usr/sbin/parted", "parted"],
        ["usr/bin/htop", "htop"],
        ["usr/sbin/iwd", "iwd"],
        ["usr/bin/grep", "grep"],
        ["sbin/cryptsetup", "cryptsetup"],
    ];

    for entry in key_binaries {
        let bin_path = entry[0];
        let pkg_name = entry[1];
        let full_path = join_path(rootfs, bin_path);

        if !is_file(full_path) {
            throw "Key binary missing: " + bin_path + " (from " + pkg_name + ")\n" +
                  "Package installation may have failed.";
        }
    }

    log("All key binaries verified");
    write_file(install_marker, ctx.version);

    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    let version_file = join_path(BUILD_DIR, ".packages-version");
    let manifest_file = join_path(BUILD_DIR, ".packages-manifest");
    let install_marker = join_path(BUILD_DIR, ".packages-installed");

    log("Cleaning packages state files...");

    if is_file(version_file) { rm(version_file); }
    if is_file(manifest_file) { rm(manifest_file); }
    if is_file(install_marker) { rm(install_marker); }

    log("Cleanup complete (rootfs not touched - run alpine cleanup for that)");
    ctx
}

// === REMOVE ===

fn remove(ctx) {
    // Can't easily remove packages - would need to rebuild rootfs
    log("To remove packages, run: recipe remove alpine && recipe install alpine");
    ctx
}
