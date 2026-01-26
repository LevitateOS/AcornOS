//! Component definitions for AcornOS.
//!
//! This module contains all the static component definitions that describe
//! what operations need to be performed to build an AcornOS system image.
//!
//! Components are organized by phase and purpose:
//! - FILESYSTEM: Create FHS directories and merged-usr symlinks
//! - BUSYBOX: Set up busybox and applet symlinks
//! - OPENRC: Set up OpenRC init system
//! - NETWORK: Network configuration and services
//! - BRANDING: AcornOS identity files (os-release, hostname, MOTD)
//! - FIRMWARE: WiFi and hardware firmware
//! - FINAL: Welcome message, live overlay, installer tools

use distro_builder::component::Phase;

use super::{
    bin, bins, copy_tree, custom, dir, dir_mode, dirs, group, openrc_conf, openrc_enable,
    openrc_scripts, sbins, symlink, user, write_file, write_file_mode, Component, CustomOp,
};

// =============================================================================
// Phase 1: Filesystem
// =============================================================================

/// Standard FHS directories.
const FHS_DIRS: &[&str] = &[
    // Core directories
    "etc",
    "home",
    "root",
    "tmp",
    "var",
    "run",
    "mnt",
    "media",
    "srv",
    "opt",
    // /usr hierarchy (merged-usr)
    "usr/bin",
    "usr/sbin",
    "usr/lib",
    "usr/lib/modules",
    "usr/share",
    "usr/local/bin",
    "usr/local/lib",
    "usr/local/share",
    // /var hierarchy
    "var/log",
    "var/tmp",
    "var/cache",
    "var/spool",
    "var/lib",
    // Device directories
    "dev",
    "proc",
    "sys",
    // Boot (for kernel, initramfs)
    "boot",
];

/// Filesystem setup component.
///
/// Creates FHS directories, merged-usr symlinks, and copies musl libc.
pub static FILESYSTEM: Component = Component {
    name: "filesystem",
    phase: Phase::Filesystem,
    ops: &[
        dirs(FHS_DIRS),
        // Merged /usr symlinks - Alpine uses merged-usr
        custom(CustomOp::CreateFhsSymlinks),
        // /tmp with sticky bit
        dir_mode("tmp", 0o1777),
        // /var/tmp with sticky bit
        dir_mode("var/tmp", 0o1777),
        // /root with restricted permissions
        dir_mode("root", 0o700),
        // CRITICAL: Copy ALL shared libraries from source rootfs
        // Host ldd (glibc) can't detect musl dependencies, so we copy everything
        // This MUST happen before any binaries are copied
        custom(CustomOp::CopyAllLibraries),
    ],
};

// =============================================================================
// Phase 2: Binaries (Busybox)
// =============================================================================

/// Busybox component.
///
/// In AcornOS, busybox provides most coreutils. Instead of copying
/// individual binaries, we copy busybox and create applet symlinks.
pub static BUSYBOX: Component = Component {
    name: "busybox",
    phase: Phase::Binaries,
    ops: &[
        // Copy busybox binary
        bin("busybox"),
        // Create all applet symlinks (includes /usr/bin/sh -> busybox)
        custom(CustomOp::CreateBusyboxApplets),
    ],
};

/// Additional binaries not provided by busybox.
///
/// These are standalone binaries that need to be copied from
/// the Alpine rootfs with their library dependencies.
const ADDITIONAL_BINS: &[&str] = &[
    // Shells
    "bash",
    // GNU coreutils (for compatibility)
    "coreutils",
    // Text editors
    "vim",
    // System utilities
    "less",
    "htop",
    // SSH utilities (for sshd)
    "ssh-keygen",
];

const ADDITIONAL_SBINS: &[&str] = &[
    // OpenRC init system (CRITICAL - these must be copied before OPENRC component)
    "openrc",
    "openrc-init",
    "openrc-run",
    "openrc-shutdown",
    // OpenRC utilities (used by init scripts)
    "start-stop-daemon",
    // Login (required for inittab)
    "agetty",
    // Partitioning
    "fdisk",
    "parted",
    "sgdisk",
    // Filesystems
    "mkfs.ext4",
    "mkfs.fat",
    "mkfs.btrfs",
    "fsck",
    "fsck.ext4",
    "blkid",
    // Device mapper
    "cryptsetup",
    "lvm",
    // Network
    "ip",
    "dhcpcd",
    // Services
    "chronyd",
    "sshd",
];

/// Additional utilities component.
///
/// Binaries beyond what busybox provides.
pub static UTILITIES: Component = Component {
    name: "utilities",
    phase: Phase::Binaries,
    ops: &[bins(ADDITIONAL_BINS), sbins(ADDITIONAL_SBINS)],
};

// =============================================================================
// Phase 3: Init (OpenRC)
// =============================================================================

/// OpenRC init scripts to copy from source.
const OPENRC_SCRIPTS: &[&str] = &[
    "hostname",
    "networking",
    "bootmisc",
    "devfs",
    "dmesg",
    "fsck",
    "hwclock",
    "hwdrivers",
    "killprocs",
    "localmount",
    "modules",
    "mount-ro",
    "mtab",
    "procfs",
    "root",
    "savecache",
    "seedrng",
    "sysctl",
    "sysfs",
    "swap",
    "swclock",
    // Device manager (provides 'dev' service needed by hwdrivers)
    "mdev",
    // Note: urandom doesn't exist in Alpine - seedrng handles random seed
    // Services
    "sshd",
    "chronyd",
    "dhcpcd",
    "iwd",
    "local",
];

/// OpenRC runlevel directories.
const RUNLEVEL_DIRS: &[&str] = &[
    "etc/runlevels/sysinit",
    "etc/runlevels/boot",
    "etc/runlevels/default",
    "etc/runlevels/nonetwork",
    "etc/runlevels/shutdown",
];

/// OpenRC component.
///
/// Sets up the OpenRC init system with runlevels and services.
pub static OPENRC: Component = Component {
    name: "openrc",
    phase: Phase::Init,
    ops: &[
        // OpenRC directories
        dir("etc/init.d"),
        dir("etc/conf.d"),
        dirs(RUNLEVEL_DIRS),
        // CRITICAL: /sbin/init must point to busybox (not openrc-init!)
        // Busybox init reads /etc/inittab and:
        // 1. Runs openrc via ::sysinit: lines
        // 2. Spawns gettys via ::respawn: lines
        // openrc-init does NOT properly handle inittab respawn lines
        symlink("sbin/init", "/bin/busybox"),
        // Copy OpenRC support scripts and binaries
        // These are REQUIRED for OpenRC to function
        copy_tree("usr/libexec/rc"),
        // Copy OpenRC configuration
        copy_tree("etc/rc.conf"),
        // Copy init scripts
        openrc_scripts(OPENRC_SCRIPTS),
        // Enable boot services (sysinit)
        openrc_enable("devfs", "sysinit"),
        openrc_enable("mdev", "sysinit"), // Device manager - provides 'dev' service, creates /dev/ttyS0 etc.
        openrc_enable("dmesg", "sysinit"),
        openrc_enable("hwdrivers", "sysinit"),
        openrc_enable("modules", "sysinit"),
        openrc_enable("sysfs", "sysinit"),
        openrc_enable("procfs", "sysinit"),
        // Enable boot services (boot)
        openrc_enable("hostname", "boot"),
        openrc_enable("bootmisc", "boot"),
        openrc_enable("hwclock", "boot"),
        openrc_enable("sysctl", "boot"),
        openrc_enable("localmount", "boot"),
        openrc_enable("fsck", "boot"),
        openrc_enable("root", "boot"),
        openrc_enable("swap", "boot"),
        openrc_enable("seedrng", "boot"),
        // Enable shutdown services
        openrc_enable("killprocs", "shutdown"),
        openrc_enable("mount-ro", "shutdown"),
        openrc_enable("savecache", "shutdown"),
    ],
};

/// Device manager component.
///
/// Sets up eudev (standalone udev fork) for device management.
/// Note: mdev from busybox is too limited for a daily driver.
pub static DEVICE_MANAGER: Component = Component {
    name: "eudev",
    phase: Phase::Init,
    ops: &[
        // Copy udev rules
        copy_tree("etc/udev"),
        copy_tree("usr/lib/udev"),
        // Set up device manager
        custom(CustomOp::SetupDeviceManager),
    ],
};

/// Kernel modules component.
///
/// Copies kernel modules from staging and runs depmod.
pub static MODULES: Component = Component {
    name: "modules",
    phase: Phase::Init,
    ops: &[
        // Copy kernel modules to squashfs root
        custom(CustomOp::CopyModules),
    ],
};

// =============================================================================
// Phase 5: Services
// =============================================================================

/// Basic /etc/network/interfaces for Alpine networking.
const NETWORK_INTERFACES: &str = "# /etc/network/interfaces - AcornOS
auto lo
iface lo inet loopback

# Enable DHCP on common interface names
# eth0 for QEMU virtio-net, enp* for real hardware
auto eth0
iface eth0 inet dhcp
";

/// Network component.
pub static NETWORK: Component = Component {
    name: "network",
    phase: Phase::Services,
    ops: &[
        // Network configuration directories
        dir("etc/network"),
        dir("etc/network/if-down.d"),
        dir("etc/network/if-post-down.d"),
        dir("etc/network/if-pre-up.d"),
        dir("etc/network/if-up.d"),
        // Basic network interfaces file
        write_file("etc/network/interfaces", NETWORK_INTERFACES),
        // Copy network configuration
        copy_tree("etc/network"),
        // Enable networking service
        openrc_enable("networking", "boot"),
        // Enable dhcpcd for automatic IP
        openrc_enable("dhcpcd", "default"),
        // DHCP configuration
        openrc_conf(
            "dhcpcd",
            "# DHCP client configuration\ndhcpcd_args=\"--quiet\"\n",
        ),
        // WiFi support (iwd)
        // DISABLED: iwd needs dbus which isn't installed
        // TODO: Install dbus and re-enable
        dir("var/lib/iwd"),
        // openrc_enable("iwd", "default"),
    ],
};

/// SSH component.
pub static SSH: Component = Component {
    name: "ssh",
    phase: Phase::Services,
    ops: &[
        // SSH directories
        dir("etc/ssh"),
        dir_mode("var/empty/sshd", 0o755),
        dir_mode("run/sshd", 0o755),
        // Copy SSH configuration
        copy_tree("etc/ssh"),
        // sshd user and group
        group("sshd", 22),
        user("sshd", 22, 22, "/var/empty/sshd", "/sbin/nologin"),
        // DISABLED: sshd needs more files (/usr/lib/ssh/sshd-session)
        // TODO: Fix sshd dependencies and re-enable
        // openrc_enable("sshd", "default"),
    ],
};

/// Time synchronization component.
pub static CHRONY: Component = Component {
    name: "chrony",
    phase: Phase::Services,
    ops: &[
        // Chrony directories
        dir("var/lib/chrony"),
        dir("var/log/chrony"),
        // Copy chrony configuration
        copy_tree("etc/chrony"),
        // chrony user
        group("chrony", 123),
        user("chrony", 123, 123, "/var/lib/chrony", "/sbin/nologin"),
        // DISABLED: chronyd needs config file
        // TODO: Create /etc/chrony/chrony.conf and re-enable
        // openrc_enable("chronyd", "default"),
    ],
};

// =============================================================================
// Phase 6: Config
// =============================================================================

/// AcornOS os-release content.
const OS_RELEASE: &str = r#"NAME="AcornOS"
ID=acornos
ID_LIKE=alpine
VERSION_ID=1.0
PRETTY_NAME="AcornOS"
HOME_URL="https://levitateos.org/acorn"
BUG_REPORT_URL="https://github.com/levitateos/levitateos/issues"
"#;

/// AcornOS MOTD.
const MOTD: &str = r#"
    _                          ___  ____
   / \   ___ ___  _ __ _ __   / _ \/ ___|
  / _ \ / __/ _ \| '__| '_ \ | | | \___ \
 / ___ \ (_| (_) | |  | | | || |_| |___) |
/_/   \_\___\___/|_|  |_| |_| \___/|____/

Welcome to AcornOS!

Documentation: https://levitateos.org/acorn/docs
Source code:   https://github.com/levitateos/levitateos

"#;

/// AcornOS issue (login prompt).
const ISSUE: &str = "AcornOS \\n \\l\n\n";

/// Branding component.
///
/// Sets up AcornOS identity files (os-release, hostname, MOTD).
pub static BRANDING: Component = Component {
    name: "branding",
    phase: Phase::Config,
    ops: &[
        // OS identity
        write_file("etc/os-release", OS_RELEASE),
        write_file("etc/hostname", "acornos\n"),
        write_file("etc/motd", MOTD),
        write_file("etc/issue", ISSUE),
        // Hosts file
        write_file(
            "etc/hosts",
            "127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\tacornos\n",
        ),
        // Create /etc configuration files
        custom(CustomOp::CreateEtcFiles),
        // Security configuration (login.defs, doas.conf)
        custom(CustomOp::CreateSecurityConfig),
    ],
};

/// Base inittab content (standard login, no autologin).
/// This is for installed systems. LIVE_FINAL overrides this with autologin.
const BASE_INITTAB: &str = "# /etc/inittab - AcornOS\n\n\
::sysinit:/sbin/openrc sysinit\n\
::sysinit:/sbin/openrc boot\n\
::wait:/sbin/openrc default\n\n\
# Standard login on TTYs (no autologin for installed systems)\n\
tty1::respawn:/sbin/agetty --noclear tty1 linux\n\
tty2::respawn:/sbin/agetty tty2 linux\n\
tty3::respawn:/sbin/agetty tty3 linux\n\
tty4::respawn:/sbin/agetty tty4 linux\n\
tty5::respawn:/sbin/agetty tty5 linux\n\
tty6::respawn:/sbin/agetty tty6 linux\n\n\
# Serial console\n\
ttyS0::respawn:/sbin/agetty -L 115200 ttyS0 vt100\n\n\
::shutdown:/sbin/openrc shutdown\n\
::ctrlaltdel:/sbin/reboot\n";

/// System configuration component.
pub static SYSCONFIG: Component = Component {
    name: "sysconfig",
    phase: Phase::Config,
    ops: &[
        // fstab (minimal for live)
        write_file(
            "etc/fstab",
            "# /etc/fstab - AcornOS\n\
             # <device>    <mount>    <type>    <options>    <dump> <pass>\n\
             proc         /proc      proc      defaults     0      0\n\
             sysfs        /sys       sysfs     defaults     0      0\n\
             devpts       /dev/pts   devpts    defaults     0      0\n\
             tmpfs        /tmp       tmpfs     defaults     0      0\n",
        ),
        // Shells
        write_file(
            "etc/shells",
            "/bin/sh\n/bin/ash\n/bin/bash\n/usr/bin/bash\n",
        ),
        // CRITICAL: Base inittab for all systems (installed and live)
        // LIVE_FINAL overrides this with autologin version for live ISO
        write_file_mode("etc/inittab", BASE_INITTAB, 0o644),
        // Copy timezone data
        custom(CustomOp::CopyTimezoneData),
    ],
};

// =============================================================================
// Phase 8: Firmware
// =============================================================================

/// Firmware component.
pub static FIRMWARE: Component = Component {
    name: "firmware",
    phase: Phase::Firmware,
    ops: &[
        // WiFi firmware (minimum needed for most laptops)
        custom(CustomOp::CopyWifiFirmware),
        // All firmware for daily driver support
        custom(CustomOp::CopyAllFirmware),
    ],
};

// =============================================================================
// Phase 9: Final
// =============================================================================

/// Final setup component for live ISO.
pub static LIVE_FINAL: Component = Component {
    name: "live-final",
    phase: Phase::Final,
    ops: &[
        // Welcome message
        custom(CustomOp::CreateWelcomeMessage),
        // Live overlay for tmpfs
        custom(CustomOp::CreateLiveOverlay),
        // Installer tools
        custom(CustomOp::CopyRecstrap),
        // Root autologin for live (both tty1 AND serial for testing)
        write_file_mode(
            "etc/inittab",
            "# /etc/inittab - AcornOS Live\n\n\
             ::sysinit:/sbin/openrc sysinit\n\
             ::sysinit:/sbin/openrc boot\n\
             ::wait:/sbin/openrc default\n\n\
             # Autologin as root on tty1 (VGA console)\n\
             tty1::respawn:/sbin/agetty --autologin root --noclear tty1 linux\n\
             tty2::respawn:/sbin/agetty tty2 linux\n\
             tty3::respawn:/sbin/agetty tty3 linux\n\n\
             # Serial console with autologin for QEMU testing\n\
             ttyS0::respawn:/sbin/agetty --autologin root -L 115200 ttyS0 vt100\n\n\
             ::shutdown:/sbin/openrc shutdown\n\
             ::ctrlaltdel:/sbin/reboot\n",
            0o644,
        ),
    ],
};

// =============================================================================
// All Components (for build_system)
// =============================================================================

/// All components in phase order.
///
/// This list is used by `build_system()` to execute all components.
pub static ALL_COMPONENTS: &[&Component] = &[
    // Phase 1: Filesystem
    &FILESYSTEM,
    // Phase 2: Binaries
    &BUSYBOX,
    &UTILITIES,
    // Phase 3: Init
    &OPENRC,
    &DEVICE_MANAGER,
    &MODULES,
    // Phase 5: Services
    &NETWORK,
    &SSH,
    &CHRONY,
    // Phase 6: Config
    &BRANDING,
    &SYSCONFIG,
    // Phase 8: Firmware
    &FIRMWARE,
    // Phase 9: Final
    &LIVE_FINAL,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_components_have_ops() {
        // Verify all components have at least one operation
        for component in ALL_COMPONENTS {
            assert!(
                !component.ops.is_empty(),
                "Component '{}' has no operations",
                component.name
            );
        }
    }

    #[test]
    fn test_components_ordered_by_phase() {
        // Verify components are in phase order
        let mut last_phase = Phase::Filesystem;
        for component in ALL_COMPONENTS {
            assert!(
                component.phase >= last_phase,
                "Component '{}' is out of order (phase {:?} after {:?})",
                component.name,
                component.phase,
                last_phase
            );
            last_phase = component.phase;
        }
    }

    #[test]
    fn test_branding_content() {
        // Verify branding content is correct
        assert!(OS_RELEASE.contains("AcornOS"));
        assert!(MOTD.contains("AcornOS"));
        assert!(!OS_RELEASE.contains("Alpine")); // Should NOT say Alpine
    }
}
