//! AcornOS component definitions.
//!
//! # Status: SKELETON
//!
//! This module will contain AcornOS-specific component definitions
//! similar to leviso's definitions.rs.
//!
//! Key differences from LevitateOS:
//! - OpenRC service operations instead of systemd
//! - busybox applet setup instead of GNU coreutils
//! - mdev device manager instead of udev
//! - musl-specific library paths

use distro_builder::component::{Installable, Op, Phase};

/// OpenRC service operation.
///
/// AcornOS uses OpenRC instead of systemd.
/// This enum defines OpenRC-specific operations.
#[derive(Debug, Clone)]
pub enum OpenRCOp {
    /// Add service to runlevel: rc-update add <service> <runlevel>
    AddService {
        service: String,
        runlevel: String,
    },
    /// Copy OpenRC service script
    CopyService(String),
    /// Create OpenRC conf.d file
    CreateConf {
        service: String,
        content: String,
    },
}

/// Busybox applet setup operation.
#[derive(Debug, Clone)]
pub enum BusyboxOp {
    /// Create symlinks for busybox applets
    CreateAppletSymlinks(Vec<String>),
    /// Install busybox binary
    InstallBusybox,
}

/// Placeholder component for AcornOS filesystem setup.
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
        // This will be similar to LevitateOS but with:
        // - /bin/sh -> busybox instead of bash
        // - Different library paths for musl
        vec![]
    }
}

/// Placeholder component for OpenRC setup.
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
        // - Create runlevel directories
        vec![]
    }
}
