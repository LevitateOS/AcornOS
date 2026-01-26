//! AcornOS component system.
//!
//! This module provides a declarative component system for building AcornOS
//! system images. Components are defined as data structures that describe
//! WHAT needs to happen, not HOW. The executor interprets these definitions.
//!
//! # Architecture
//!
//! ```text
//! Component Definition (DATA)     →     Executor (LOGIC)
//! ─────────────────────────────        ─────────────────
//! OPENRC = Component {                 for op in component.ops {
//!   ops: [                               execute_op(ctx, op)?;
//!     dir("etc/init.d"),               }
//!     openrc_enable("networking", "boot"),
//!     custom(CreateOsRelease),
//!   ]
//! }
//! ```
//!
//! # Key Differences from LevitateOS
//!
//! | Aspect | LevitateOS | AcornOS |
//! |--------|-----------|---------|
//! | Init | systemd units | OpenRC services |
//! | Coreutils | GNU binaries | busybox applets |
//! | Device manager | udev | mdev (busybox) or eudev |
//! | Shell | bash | ash (busybox) |
//! | Library paths | /usr/lib64 (glibc) | /usr/lib (musl) |

pub mod builder;
pub mod context;
pub mod custom;
pub mod definitions;
pub mod executor;

pub use builder::build_system;
pub use context::BuildContext;
pub use definitions::*;

// Re-export from distro-builder for convenience
pub use distro_builder::component::{Installable, Phase};

/// A system component that can be installed.
///
/// Components are immutable data describing what operations need to be
/// performed to set up a particular system service.
#[derive(Debug, Clone)]
pub struct Component {
    /// Human-readable name for logging.
    pub name: &'static str,
    /// Build phase (determines ordering).
    pub phase: Phase,
    /// Operations to perform.
    pub ops: &'static [Op],
}

impl Installable for Component {
    fn name(&self) -> &str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn ops(&self) -> Vec<distro_builder::component::Op> {
        // Convert our static ops to distro_builder ops for the trait
        // Note: We handle our own ops in our executor
        vec![]
    }
}

impl Installable for &Component {
    fn name(&self) -> &str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn ops(&self) -> Vec<distro_builder::component::Op> {
        vec![]
    }
}

/// Operations that can be performed during component installation.
///
/// Each variant represents a single atomic operation. The executor
/// handles the actual implementation, ensuring consistent behavior.
///
/// ALL operations are required. If something is listed, it must exist.
/// There is no "optional" - this is a daily driver OS, not a toy.
#[derive(Debug, Clone)]
pub enum Op {
    // ─────────────────────────────────────────────────────────────────────
    // Directory operations
    // ─────────────────────────────────────────────────────────────────────
    /// Create a directory (uses create_dir_all).
    Dir(&'static str),

    /// Create a directory with specific permissions.
    DirMode(&'static str, u32),

    /// Create multiple directories at once.
    Dirs(&'static [&'static str]),

    // ─────────────────────────────────────────────────────────────────────
    // File operations
    // ─────────────────────────────────────────────────────────────────────
    /// Write a file with given content.
    WriteFile(&'static str, &'static str),

    /// Write a file with specific permissions.
    WriteFileMode(&'static str, &'static str, u32),

    /// Create a symlink (link_path, target).
    Symlink(&'static str, &'static str),

    /// Copy a single file from source to staging. Fails if not found.
    CopyFile(&'static str),

    /// Copy a directory tree from source to staging.
    CopyTree(&'static str),

    // ─────────────────────────────────────────────────────────────────────
    // Binary operations (simplified for busybox-based system)
    // ─────────────────────────────────────────────────────────────────────
    /// Copy a binary with library dependencies to /usr/bin.
    /// For busybox applets, this creates a symlink instead.
    Bin(&'static str),

    /// Copy a binary to /usr/sbin.
    Sbin(&'static str),

    /// Copy multiple binaries to /usr/bin.
    Bins(&'static [&'static str]),

    /// Copy multiple binaries to /usr/sbin.
    Sbins(&'static [&'static str]),

    // ─────────────────────────────────────────────────────────────────────
    // OpenRC operations (AcornOS-specific)
    // ─────────────────────────────────────────────────────────────────────
    /// Enable an OpenRC service in a runlevel.
    ///
    /// Creates symlink: /etc/runlevels/<runlevel>/<service> -> /etc/init.d/<service>
    OpenrcEnable(&'static str, &'static str),

    /// Copy OpenRC init scripts from source.
    OpenrcScripts(&'static [&'static str]),

    /// Write an OpenRC conf.d configuration file.
    OpenrcConf(&'static str, &'static str),

    // ─────────────────────────────────────────────────────────────────────
    // User/group operations
    // ─────────────────────────────────────────────────────────────────────
    /// Ensure a user exists in passwd file.
    User {
        name: &'static str,
        uid: u32,
        gid: u32,
        home: &'static str,
        shell: &'static str,
    },

    /// Ensure a group exists in group file.
    Group { name: &'static str, gid: u32 },

    // ─────────────────────────────────────────────────────────────────────
    // Custom operations (dispatch to custom modules)
    // ─────────────────────────────────────────────────────────────────────
    /// Run a custom operation.
    Custom(CustomOp),
}

/// Custom operations that require imperative code.
///
/// These operations have complex logic that doesn't fit the declarative
/// pattern. Each variant maps to a function in the custom module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomOp {
    /// Create FHS symlinks (merged /usr).
    CreateFhsSymlinks,
    /// Create AcornOS /etc/os-release.
    CreateOsRelease,
    /// Create AcornOS MOTD and issue files.
    CreateBranding,
    /// Create busybox applet symlinks.
    CreateBusyboxApplets,
    /// Setup mdev or eudev device manager.
    SetupDeviceManager,
    /// Copy kernel modules and run depmod.
    CopyModules,
    /// Run depmod for kernel modules.
    RunDepmod,
    /// Copy WiFi firmware.
    CopyWifiFirmware,
    /// Copy all firmware.
    CopyAllFirmware,
    /// Create /etc configuration files.
    CreateEtcFiles,
    /// Create security configuration (login.defs, etc.).
    CreateSecurityConfig,
    /// Copy timezone data.
    CopyTimezoneData,
    /// Create welcome message for live ISO.
    CreateWelcomeMessage,
    /// Setup live overlay directory.
    CreateLiveOverlay,
    /// Copy recstrap installer tools.
    CopyRecstrap,
    /// Copy all shared libraries from source rootfs.
    /// Required because host glibc ldd can't analyze musl binaries.
    CopyAllLibraries,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for readable component definitions
// ─────────────────────────────────────────────────────────────────────────────

/// Create a directory.
pub const fn dir(path: &'static str) -> Op {
    Op::Dir(path)
}

/// Create a directory with specific mode.
pub const fn dir_mode(path: &'static str, mode: u32) -> Op {
    Op::DirMode(path, mode)
}

/// Create multiple directories.
pub const fn dirs(paths: &'static [&'static str]) -> Op {
    Op::Dirs(paths)
}

/// Write a file.
pub const fn write_file(path: &'static str, content: &'static str) -> Op {
    Op::WriteFile(path, content)
}

/// Write a file with permissions.
pub const fn write_file_mode(path: &'static str, content: &'static str, mode: u32) -> Op {
    Op::WriteFileMode(path, content, mode)
}

/// Create a symlink.
pub const fn symlink(link: &'static str, target: &'static str) -> Op {
    Op::Symlink(link, target)
}

/// Copy a file from source. Fails if not found.
pub const fn copy_file(path: &'static str) -> Op {
    Op::CopyFile(path)
}

/// Copy a directory tree from source.
pub const fn copy_tree(path: &'static str) -> Op {
    Op::CopyTree(path)
}

/// Copy a binary to /usr/bin.
pub const fn bin(name: &'static str) -> Op {
    Op::Bin(name)
}

/// Copy a binary to /usr/sbin.
pub const fn sbin(name: &'static str) -> Op {
    Op::Sbin(name)
}

/// Copy multiple binaries to /usr/bin.
pub const fn bins(names: &'static [&'static str]) -> Op {
    Op::Bins(names)
}

/// Copy multiple binaries to /usr/sbin.
pub const fn sbins(names: &'static [&'static str]) -> Op {
    Op::Sbins(names)
}

/// Enable an OpenRC service in a runlevel.
pub const fn openrc_enable(service: &'static str, runlevel: &'static str) -> Op {
    Op::OpenrcEnable(service, runlevel)
}

/// Copy OpenRC init scripts.
pub const fn openrc_scripts(scripts: &'static [&'static str]) -> Op {
    Op::OpenrcScripts(scripts)
}

/// Write an OpenRC conf.d file.
pub const fn openrc_conf(service: &'static str, content: &'static str) -> Op {
    Op::OpenrcConf(service, content)
}

/// Ensure a user exists.
pub const fn user(
    name: &'static str,
    uid: u32,
    gid: u32,
    home: &'static str,
    shell: &'static str,
) -> Op {
    Op::User {
        name,
        uid,
        gid,
        home,
        shell,
    }
}

/// Ensure a group exists.
pub const fn group(name: &'static str, gid: u32) -> Op {
    Op::Group { name, gid }
}

/// Run a custom operation.
pub const fn custom(op: CustomOp) -> Op {
    Op::Custom(op)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_helpers() {
        // Test that helper functions create expected Op variants
        assert!(matches!(dir("etc"), Op::Dir("etc")));
        assert!(matches!(
            write_file("etc/hostname", "acornos"),
            Op::WriteFile("etc/hostname", "acornos")
        ));
        assert!(matches!(
            openrc_enable("networking", "boot"),
            Op::OpenrcEnable("networking", "boot")
        ));
    }

    #[test]
    fn test_custom_ops() {
        assert!(matches!(
            custom(CustomOp::CreateOsRelease),
            Op::Custom(CustomOp::CreateOsRelease)
        ));
    }
}
