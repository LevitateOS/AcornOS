//! AcornOS ISO builder library.
//!
//! AcornOS is a sibling distribution to LevitateOS, built on:
//! - **Alpine Linux** packages (APKs)
//! - **OpenRC** init system
//! - **musl** C library
//! - **busybox** coreutils
//!
//! # Status
//!
//! This library is a **structural skeleton**. Most functionality
//! is not yet implemented.
//!
//! # Architecture
//!
//! ```text
//! AcornOS (this crate)
//!     │
//!     ├── config.rs      DistroConfig implementation
//!     ├── extract.rs     Alpine APK extraction (placeholder)
//!     └── component/     OpenRC-specific components (placeholder)
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

pub mod component;
pub mod config;
pub mod extract;

pub use config::AcornConfig;
