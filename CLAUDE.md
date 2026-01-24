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

## Status: SKELETON

This crate is a **structural skeleton only**. Commands return `unimplemented!()`.

### What's Implemented
- CLI structure (`cargo run -- status` works)
- distro-spec/acorn/ configuration (boot modules, paths, services)
- DistroConfig trait implementation

### What's NOT Implemented (Future Work)
- Alpine APK extraction
- OpenRC service setup
- Component definitions (list of Alpine packages)
- Initramfs building (with mdev)
- ISO creation
- QEMU runner

---

## Development Roadmap

### Phase 1: Alpine Rootfs (NOT STARTED)
- [ ] Download Alpine minirootfs
- [ ] Extract to downloads/rootfs
- [ ] Verify basic structure

### Phase 2: Package Management (NOT STARTED)
- [ ] Integrate with Alpine APK repos
- [ ] Download required packages
- [ ] Extract packages to staging

### Phase 3: OpenRC Setup (NOT STARTED)
- [ ] Copy OpenRC binary and libraries
- [ ] Create runlevel structure
- [ ] Enable boot services (networking, chronyd, sshd)

### Phase 4: Initramfs (NOT STARTED)
- [ ] Use busybox from Alpine
- [ ] Set up mdev or eudev
- [ ] Create init script (similar to leviso but OpenRC)

### Phase 5: ISO Creation (NOT STARTED)
- [ ] Reuse distro-builder abstractions
- [ ] Create GRUB config
- [ ] Build with xorriso

---

## Commands

```bash
cargo run -- status    # Shows skeleton status
cargo run -- build     # NOT IMPLEMENTED
cargo run -- run       # NOT IMPLEMENTED
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

## Anti-Patterns to Avoid

### "It's Alpine, so it should be minimal"
**WRONG.** AcornOS is a **daily driver desktop**. Alpine is the BASE, not the GOAL.

We use Alpine because:
1. musl + busybox = smaller attack surface
2. OpenRC = simpler init
3. Different from LevitateOS (user choice)

We do NOT use Alpine to be "minimal". A complete desktop needs:
- Firmware support
- Desktop services (dbus, elogind)
- Audio/video stack
- Full hardware support

### "Skip X because Alpine doesn't need it"
**WRONG.** If LevitateOS has it and users need it, AcornOS needs it too.

---

## Resources

- [Alpine Linux Wiki](https://wiki.alpinelinux.org/)
- [OpenRC Documentation](https://wiki.gentoo.org/wiki/OpenRC)
- [musl libc](https://musl.libc.org/)
- [busybox](https://busybox.net/)
