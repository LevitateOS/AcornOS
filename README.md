# AcornOS

AcornOS ISO builder - a sibling distribution to [LevitateOS](https://github.com/LevitateOS/LevitateOS).

## What is AcornOS?

AcornOS is a **daily driver Linux distribution** built on:

| Component | Technology |
|-----------|------------|
| Base packages | Alpine Linux APKs |
| Init system | OpenRC |
| C library | musl |
| Coreutils | busybox |
| Shell | ash (busybox) |

Both AcornOS and LevitateOS are:
- **Complete daily driver desktops** (NOT embedded, NOT minimal)
- **Competing with Arch Linux** in philosophy
- **User-controlled** - you are your own package maintainer

## Comparison

| | LevitateOS | AcornOS |
|---|-----------|---------|
| Base | Rocky Linux RPMs | Alpine APKs |
| Init | systemd | OpenRC |
| libc | glibc | musl |
| Coreutils | GNU | busybox |
| Shell | bash | ash |

Choose LevitateOS for maximum compatibility. Choose AcornOS for a smaller attack surface and simpler init.

## Status

**Alpha.** Produces a bootable ISO with an EROFS rootfs and tiny initramfs, and can boot in QEMU.

Specs are defined in `distro-spec/src/acorn/` (packages, services, paths, UKI entries).

## Usage

```bash
cd AcornOS

# Show status / next steps
cargo run -- status

# Validate host tools and prerequisites
cargo run -- preflight

# Download Alpine ISO + apk-tools, install package tiers
cargo run -- download alpine

# Build (kernel may be reused/stolen; full kernel build requires explicit confirmation)
cargo run -- build

# Boot in QEMU
cargo run -- run

# Automated headless boot smoke test
cargo run -- test
```

## Architecture

```
AcornOS/
├── src/
│   ├── main.rs        # CLI entrypoint
│   ├── config.rs      # DistroConfig implementation
│   ├── artifact/      # rootfs/initramfs/ISO builders
│   ├── component/     # OpenRC components and wiring
│   ├── qemu.rs        # QEMU runner
│   └── rebuild.rs     # Rebuild detection + caching
├── deps/              # .rhai dependency recipes (Alpine, packages, tools)
└── profile/           # Live overlay content injected into ISO
```

## Related Projects

- [LevitateOS](https://github.com/LevitateOS/LevitateOS) - Parent project
- [leviso](https://github.com/LevitateOS/leviso) - LevitateOS ISO builder (reference implementation)
- [distro-spec](https://github.com/LevitateOS/distro-spec) - Distribution specifications
- [distro-builder](https://github.com/LevitateOS/distro-builder) - Shared build infrastructure

## Contributing

Key areas:

1. **Package tiers**: tune `distro-spec/src/acorn/packages.rs` (daily-driver defaults)
2. **Services**: OpenRC enablement and defaults in `distro-spec/src/acorn/services.rs`
3. **Profiles/overlays**: live behavior in `AcornOS/profile/`
4. **Boot/testing**: keep QEMU smoke tests and `testing/install-tests/` checkpoints green

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
