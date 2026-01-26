# CLAUDE.md - AcornOS

## ⛔ STOP. READ. THEN ACT.

Before writing code for AcornOS:
1. Read the parent project's CLAUDE.md (/home/vince/Projects/LevitateOS/CLAUDE.md)
2. Read leviso's code to understand the patterns
3. AcornOS follows the SAME patterns, just with different tools

---

## What is AcornOS?

**AcornOS is a daily driver Linux distribution, sibling to LevitateOS.**

| | LevitateOS | AcornOS |
|---|-----------|---------|
| Base packages | Rocky Linux RPMs | Alpine APKs |
| Init system | systemd | OpenRC |
| C library | glibc | musl |
| Coreutils | GNU | busybox |
| Shell | bash | ash (busybox) |
| Device manager | udev | mdev (busybox) |

Both are:
- **Daily driver desktops** (NOT minimal, NOT embedded)
- **Competing with Arch Linux**
- **Complete and functional out of the box**

---

## Status: BUILD PIPELINE COMPLETE

The build pipeline is fully implemented. End-to-end testing needed.

### What's Implemented
- CLI structure (all commands work)
- distro-spec/acorn/ configuration (boot modules, paths, services)
- DistroConfig trait implementation
- Alpine ISO download and extraction
- Squashfs builder
- Initramfs builder (tiny, OpenRC-compatible)
- ISO builder (UEFI bootable)
- QEMU runner

### What Needs Testing
- Full end-to-end build (`acornos build`)
- Boot to OpenRC login prompt

---

## Commands

```bash
cargo run -- status           # Show build status
cargo run -- download         # Download Alpine Extended ISO (~1GB)
cargo run -- extract          # Extract ISO and create rootfs
cargo run -- build squashfs   # Build only squashfs
cargo run -- build            # Full build (squashfs + initramfs + ISO)
cargo run -- initramfs        # Rebuild initramfs only
cargo run -- iso              # Rebuild ISO only
cargo run -- run              # Boot ISO in QEMU
```

## Key Differences from leviso

### Init System
```rust
// LevitateOS (systemd)
Op::Enable("sshd.service", Target::MultiUser)

// AcornOS (OpenRC)
OpenRCOp::AddService { service: "sshd", runlevel: "default" }
```

### Shell
```rust
// LevitateOS
distro_spec::levitate::DEFAULT_SHELL  // "/bin/bash"

// AcornOS
distro_spec::acorn::DEFAULT_SHELL     // "/bin/ash"
```

### Device Manager
- LevitateOS uses udev (from systemd)
- AcornOS will use mdev (busybox) or eudev (standalone udev fork)

### Binary Sources
- LevitateOS: `/downloads/rootfs/` (Rocky RPM extract)
- AcornOS: `/downloads/rootfs/` (Alpine APK extract)

---

## Critical: AcornOS is NOT Alpine

**AcornOS is its own distribution.** It uses Alpine's APK repositories as a **package source**, just like LevitateOS uses Rocky Linux RPMs as a package source.

| Distro | Package Source | Identity |
|--------|----------------|----------|
| LevitateOS | Rocky Linux RPMs | LevitateOS (NOT Rocky rebrand) |
| AcornOS | Alpine APKs | AcornOS (NOT Alpine rebrand) |

The rootfs should show "AcornOS", not "Alpine". The MOTD should say "Welcome to AcornOS", not "Welcome to Alpine".

We use Alpine's packages because:
1. musl + busybox = smaller attack surface
2. APK format is simple and well-documented
3. Large package repository
4. Compatible with OpenRC

---

## Anti-Patterns to Avoid

### "It's Alpine, so it should be minimal"
**WRONG.** AcornOS is a **daily driver desktop**. Alpine is a package SOURCE, not the GOAL.

We do NOT use Alpine to be "minimal". A complete desktop needs:
- Firmware support
- Desktop services (dbus, elogind)
- Audio/video stack
- Full hardware support

### "Skip X because Alpine doesn't need it"
**WRONG.** If LevitateOS has it and users need it, AcornOS needs it too.

### "Just extract Alpine rootfs and rebrand it"
**WRONG.** AcornOS builds its own rootfs by installing packages from Alpine repos, with AcornOS-specific configuration, branding, and customization.

---

## Resources

- [Alpine Linux Wiki](https://wiki.alpinelinux.org/)
- [OpenRC Documentation](https://wiki.gentoo.org/wiki/OpenRC)
- [musl libc](https://musl.libc.org/)
- [busybox](https://busybox.net/)
