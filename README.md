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

**SKELETON** - This is Step 1 of ~50 steps to a bootable AcornOS.

```bash
$ cargo run -- status
AcornOS Builder Status
======================

Status: SKELETON - Not yet implemented

AcornOS is a sibling distribution to LevitateOS:
  - Alpine Linux base (musl, busybox)
  - OpenRC init system
  - Daily driver desktop (NOT minimal)

Next steps:
  1. Implement Alpine APK extraction
  2. Create OpenRC service components
  3. Build initramfs with mdev
  4. Create bootable ISO
```

### Implemented
- CLI structure with subcommands
- `status` command
- DistroConfig implementation using distro-spec::acorn
- Placeholder modules for future implementation

### Not Yet Implemented
- Alpine APK extraction
- OpenRC service setup
- Component definitions
- Initramfs building
- ISO creation
- QEMU runner

## Usage

```bash
# Show status (the only working command)
cargo run -- status

# These return unimplemented!() for now:
cargo run -- build      # Build complete ISO
cargo run -- initramfs  # Build initramfs only
cargo run -- iso        # Build ISO only
cargo run -- run        # Run in QEMU
cargo run -- download   # Download Alpine packages
cargo run -- extract    # Extract Alpine packages
```

## Architecture

```
AcornOS/
├── src/
│   ├── main.rs        # CLI with clap
│   ├── lib.rs         # Library root
│   ├── config.rs      # DistroConfig implementation
│   ├── extract.rs     # Alpine APK extraction (placeholder)
│   └── component/
│       └── mod.rs     # OpenRC-specific components (placeholder)
└── CLAUDE.md          # Development guidelines
```

## Related Projects

- [LevitateOS](https://github.com/LevitateOS/LevitateOS) - Parent project
- [leviso](https://github.com/LevitateOS/leviso) - LevitateOS ISO builder (reference implementation)
- [distro-spec](https://github.com/LevitateOS/distro-spec) - Distribution specifications
- [distro-builder](https://github.com/LevitateOS/distro-builder) - Shared build infrastructure

## Contributing

AcornOS needs significant work. Key areas:

1. **Alpine APK extraction** - Either use apk-tools or implement APK parsing
2. **OpenRC integration** - Service setup, runlevels, dependencies
3. **mdev vs eudev** - Device manager decision
4. **Desktop services** - dbus, elogind, seat management with musl

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
