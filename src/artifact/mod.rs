//! AcornOS artifact builders.
//!
//! This module contains builders for the various artifacts needed
//! to create a bootable AcornOS ISO:
//!
//! - `squashfs` - Compresses the rootfs into filesystem.squashfs
//! - `initramfs` - Creates the tiny boot initramfs
//! - `iso` - Packages everything into a bootable ISO

pub mod initramfs;
pub mod iso;
pub mod squashfs;

pub use initramfs::build_tiny_initramfs;
pub use iso::create_squashfs_iso;
pub use squashfs::build_squashfs;
