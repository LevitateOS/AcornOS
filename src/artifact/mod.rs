//! AcornOS artifact builders.
//!
//! This module contains builders for the various artifacts needed
//! to create a bootable AcornOS ISO:
//!
//! - `rootfs` - Creates the EROFS rootfs image (filesystem.erofs)
//! - `initramfs` - Creates the tiny boot initramfs
//! - `iso` - Packages everything into a bootable ISO

pub mod initramfs;
pub mod iso;
pub mod rootfs;

pub use initramfs::build_tiny_initramfs;
pub use iso::create_iso;
pub use rootfs::build_rootfs;
