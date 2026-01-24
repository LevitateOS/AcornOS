//! AcornOS component definitions.
//!
//! This module contains AcornOS-specific component definitions,
//! similar to leviso's `component/definitions.rs`.
//!
//! # Key Differences from LevitateOS
//!
//! | Aspect | LevitateOS | AcornOS |
//! |--------|-----------|---------|
//! | Init | systemd units | OpenRC services |
//! | Coreutils | GNU binaries | busybox applets |
//! | Device manager | udev | mdev (busybox) |
//! | Shell | bash | ash (busybox) |
//! | Library paths | /usr/lib64 (glibc) | /usr/lib (musl) |
//!
//! # Status
//!
//! **PLACEHOLDER** - Component definitions not yet implemented.
//!
//! # Example (future)
//!
//! ```rust,ignore
//! use acornos::component::{OpenRCComponent, BusyboxComponent};
//! use distro_builder::component::Installable;
//!
//! let openrc = OpenRCComponent;
//! let busybox = BusyboxComponent;
//!
//! for op in openrc.ops() {
//!     executor.execute(op)?;
//! }
//! ```

use distro_builder::component::{Installable, Op, Phase};

/// OpenRC service operation.
///
/// AcornOS uses OpenRC instead of systemd.
#[derive(Debug, Clone)]
pub enum OpenRCOp {
    /// Add service to runlevel.
    ///
    /// Equivalent to: `rc-update add <service> <runlevel>`
    AddService {
        /// Service name (e.g., "sshd", "networking")
        service: String,
        /// Runlevel (e.g., "boot", "default", "sysinit")
        runlevel: String,
    },

    /// Copy an OpenRC service script to /etc/init.d/
    CopyService(String),

    /// Create an OpenRC conf.d configuration file.
    ///
    /// Creates /etc/conf.d/<service> with the given content.
    CreateConf {
        /// Service name
        service: String,
        /// Configuration content
        content: String,
    },
}

/// Busybox applet setup operation.
#[derive(Debug, Clone)]
pub enum BusyboxOp {
    /// Create symlinks for busybox applets in /bin and /sbin.
    CreateAppletSymlinks(Vec<String>),

    /// Install the busybox binary to /bin/busybox.
    InstallBusybox,
}

/// Placeholder component for AcornOS filesystem setup.
///
/// # Status
///
/// **PLACEHOLDER** - Returns empty ops.
pub struct FilesystemComponent;

impl Installable for FilesystemComponent {
    fn name(&self) -> &str {
        "Filesystem"
    }

    fn phase(&self) -> Phase {
        Phase::Filesystem
    }

    fn ops(&self) -> Vec<Op> {
        // TODO: Implement AcornOS filesystem operations
        // Differences from LevitateOS:
        // - /bin/sh -> busybox (not bash)
        // - /usr/lib only (no /usr/lib64 with musl)
        // - Different FHS layout for Alpine
        vec![]
    }
}

/// Placeholder component for OpenRC setup.
///
/// # Status
///
/// **PLACEHOLDER** - Returns empty ops.
pub struct OpenRCComponent;

impl Installable for OpenRCComponent {
    fn name(&self) -> &str {
        "OpenRC"
    }

    fn phase(&self) -> Phase {
        Phase::Init
    }

    fn ops(&self) -> Vec<Op> {
        // TODO: Implement OpenRC setup
        // - Copy openrc binary and libraries
        // - Set up /etc/rc.conf
        // - Create runlevel directories (/etc/runlevels/*)
        // - Copy init scripts to /etc/init.d/
        vec![]
    }
}

/// Placeholder component for busybox setup.
///
/// # Status
///
/// **PLACEHOLDER** - Returns empty ops.
pub struct BusyboxComponent;

impl Installable for BusyboxComponent {
    fn name(&self) -> &str {
        "Busybox"
    }

    fn phase(&self) -> Phase {
        Phase::Binaries
    }

    fn ops(&self) -> Vec<Op> {
        // TODO: Implement busybox setup
        // - Copy busybox binary
        // - Create applet symlinks (ls, cat, grep, etc.)
        // - Set up /bin/sh -> busybox
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_phases() {
        assert_eq!(FilesystemComponent.phase(), Phase::Filesystem);
        assert_eq!(OpenRCComponent.phase(), Phase::Init);
        assert_eq!(BusyboxComponent.phase(), Phase::Binaries);
    }

    #[test]
    fn test_component_names() {
        assert_eq!(FilesystemComponent.name(), "Filesystem");
        assert_eq!(OpenRCComponent.name(), "OpenRC");
        assert_eq!(BusyboxComponent.name(), "Busybox");
    }
}
