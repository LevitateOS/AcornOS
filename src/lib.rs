//! AcornOS ISO builder library.
//!
//! AcornOS is a sibling distribution to LevitateOS, built on:
//! - **Alpine Linux** packages (APKs)
//! - **OpenRC** init system
//! - **musl** C library
//! - **busybox** coreutils
//!
//! # Architecture
//!
//! ```text
//! AcornOS (this crate)
//!     │
//!     ├── config.rs      DistroConfig implementation
//!     ├── extract.rs     Path definitions (download logic in deps/alpine.rhai)
//!     ├── artifact/      Build artifacts (squashfs, initramfs, ISO)
//!     ├── qemu.rs        QEMU runner
//!     └── component/     OpenRC-specific components
//!
//! Uses:
//!     ├── distro-spec::acorn    Constants, paths, services
//!     └── distro-builder        Shared build infrastructure
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use acornos::config::AcornConfig;
//! use distro_builder::DistroConfig;
//!
//! let config = AcornConfig;
//! println!("Building {} ISO", config.os_name());
//! println!("Init system: {}", config.init_system());
//! ```

pub mod artifact;
pub mod build;
pub mod cache;
pub mod component;
pub mod config;
pub mod extract;
pub mod preflight;
pub mod qemu;
pub mod rebuild;
pub mod timing;

pub use config::AcornConfig;
pub use timing::Timer;
