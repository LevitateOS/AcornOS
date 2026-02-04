//! AcornOS artifact builders.
//!
//! This module contains builders for the various artifacts needed
//! to create a bootable AcornOS ISO:
//!
//! - `rootfs` - Creates the EROFS rootfs image (filesystem.erofs)
//! - `initramfs` - Creates the tiny boot initramfs
//! - `uki` - Builds Unified Kernel Images (UKIs) for boot
//! - `iso` - Packages everything into a bootable ISO

pub mod initramfs;
pub mod iso;
pub mod rootfs;
pub mod uki;

pub use initramfs::build_tiny_initramfs;
pub use iso::create_iso;
pub use rootfs::build_rootfs;
pub use uki::{build_installed_ukis, build_live_ukis};
