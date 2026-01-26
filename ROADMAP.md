# AcornOS Daily Driver Specification

> **DOCUMENTATION NOTE:** This roadmap will be used to create the installation guide
> and user documentation. Keep descriptions clear, complete, and user-facing.

**Version:** 1.0
**Last Updated:** 2026-01-26
**Goal:** Everything a user needs to use AcornOS as their primary operating system, competing directly with Arch Linux (Alpine-based alternative to LevitateOS).

---

## What is AcornOS?

AcornOS is a **sibling distribution** to LevitateOS, sharing the same goals but with different underlying technology:

| Aspect | LevitateOS | AcornOS |
|--------|------------|---------|
| Base packages | Rocky Linux RPMs | Alpine APKs |
| Init system | systemd | OpenRC |
| C library | glibc | musl |
| Coreutils | GNU | busybox |
| Shell | bash | ash (busybox) |
| Device manager | udev | mdev (busybox) or eudev |
| Size philosophy | Complete (~400MB) | Complete (~200MB) |

**Both are daily driver desktops. Neither is "minimal."**

---

## Alpine ISO Parity Status

> **Reference:** Alpine provides a similar live ISO experience. This section tracks gaps.

### P0 - Critical Gaps (Blocking for Daily Driver)

| Gap | Impact | Status |
|-----|--------|--------|
| **Intel/AMD microcode** | CPU bugs, security vulnerabilities | ✅ IN BUILD |
| **cryptsetup (LUKS)** | No encrypted disk support | ✅ IN BUILD |
| **lvm2** | No LVM support | ✅ IN BUILD |
| **btrfs-progs** | No Btrfs support | ✅ IN BUILD |
| **eudev or mdev** | No device management | ✅ eudev IN BUILD |

### P1 - Important Gaps

| Gap | Impact | Status |
|-----|--------|--------|
| Volatile journal/log storage | Logs may fill tmpfs | ✅ CONFIGURED |
| do-not-suspend config | Live session may sleep during install | ✅ CONFIGURED |
| SSH server (openssh) | No remote installation/rescue | ✅ IN BUILD |
| pciutils (lspci) | Cannot identify PCI hardware | ✅ IN BUILD |
| usbutils (lsusb) | Cannot identify USB devices | ✅ IN BUILD |
| dmidecode | Cannot read BIOS/DMI info | ✅ IN BUILD |
| ethtool | Cannot diagnose NICs | ✅ IN BUILD |
| iwd | Only wpa_supplicant for WiFi | ✅ IN BUILD |
| wireless-regdb | WiFi may violate regulations | ✅ IN BUILD |
| sof-firmware | Modern laptop sound may not work | ✅ IN BUILD |

### What's Working (Verified)

| Feature | Status |
|---------|--------|
| Crate structure | Skeleton only |
| DistroConfig implementation | Working |
| distro-spec/acorn constants | Complete |
| OpenRC boot to login | ✅ Verified |
| Networking (DHCP) | ✅ Verified (virtio_net, dhcpcd) |

---

## How to Use This Document

- **[ ]** = Not implemented / Not tested
- **[~]** = Partially implemented / Needs work
- **[x]** = Fully implemented and tested

Each item should have:
1. A test (when testing infrastructure exists)
2. The actual functionality in the rootfs/squashfs
3. Documentation

---

## Architecture

The ISO uses a squashfs-based live environment (like Alpine, Arch, Ubuntu):

```
ISO
├── boot/
│   ├── vmlinuz           # Kernel (Alpine linux-lts or custom)
│   └── initramfs.img     # Tiny (~5MB) - mounts squashfs
├── live/
│   └── filesystem.squashfs  # COMPLETE system (~200MB compressed)
└── EFI/...               # Bootloader (GRUB or syslinux)
```

**Boot flow:**
1. Kernel + tiny initramfs boot
2. Initramfs mounts filesystem.squashfs (read-only)
3. Initramfs mounts overlay (tmpfs for writes)
4. switch_root to squashfs
5. OpenRC starts services
6. User has FULL daily driver system

**Installation flow:**
1. Boot ISO -> live environment (from squashfs)
2. Partition disk, format, mount
3. Extract squashfs to disk (or use setup-alpine equivalent)
4. Configure fstab, bootloader, users
5. Reboot into installed system

**Key insight:** The squashfs IS the complete system. Live = Installed.

---

## Installation Guide Outline

> This roadmap will be converted into user-facing installation documentation.
> Each section maps to a documentation page.

### Pre-Installation (Website)
1. **System Requirements** - CPU, RAM, storage
2. **Download ISO** - Links, SHA512 verification
3. **Create Boot Media** - dd, Ventoy, Rufus

### Live Environment (Installation Guide)
1. **Boot the ISO** - UEFI boot process
2. **Connect to Network** - `ip link`, `wpa_supplicant`, or NetworkManager
3. **Prepare Disks** - `fdisk`/`parted`, `mkfs`
4. **Extract System** - `unsquashfs` or custom installer
5. **Configure System** - fstab, hostname, users, passwords
6. **Install Bootloader** - GRUB or syslinux
7. **Reboot** - First boot into installed system

### Post-Installation (Wiki)
1. **First Boot** - What to expect
2. **Package Management** - Using `apk` or `recipe`
3. **Desktop Environment** - Installing Sway, GNOME, etc.
4. **Troubleshooting** - Common issues

---

## System Extractor

> **NOTE:** Like LevitateOS's recstrap, AcornOS needs a simple extraction tool.

### Why squashfs + extraction?

**Problems with initramfs-only approach:**
- ~400MB RAM usage just for live environment
- Need complex logic to copy networking to installed system

**Solution - squashfs architecture:**
- Single source of truth: filesystem.squashfs has EVERYTHING
- Less RAM: squashfs reads from disk, not all in RAM
- Simple installation: just unsquash to disk
- **Live = Installed:** exact same files

### What the extractor does

```sh
# Option 1: Direct unsquashfs
unsquashfs -f -d /mnt /media/cdrom/live/filesystem.squashfs

# Option 2: Custom tool (like recstrap)
acorn-strap /mnt
```

User does EVERYTHING else manually (like Arch/Alpine):
- Partitioning (fdisk, parted)
- Formatting (mkfs.ext4, mkfs.fat)
- Mounting (/mnt, /mnt/boot)
- fstab generation
- Bootloader (grub-install or syslinux)
- Password (passwd)
- Users (adduser)
- Timezone, locale, hostname

### Squashfs architecture

```
ISO (~250MB):
├── initramfs.img (~5MB) - tiny, just mounts squashfs
└── live/filesystem.squashfs (~200MB compressed)
    ├── All binaries (busybox, OpenRC, util-linux...)
    ├── NetworkManager OR ifupdown + wpa_supplicant
    ├── ALL firmware (WiFi, GPU, sound, BT)
    ├── recipe package manager (optional)
    └── Installation tools

Live boot: squashfs mounted + overlay for writes
Installation: unsquash squashfs to /mnt
Result: Live = Installed (same files!)
```

### Implementation status

**Squashfs builder:**
- [x] Create squashfs build module (`AcornOS/src/artifact/squashfs.rs`)
- [x] Include ALL binaries from Alpine packages (via `acornos extract`)
- [~] Include NetworkManager or ifupdown (ifupdown via Alpine packages)
- [~] Include firmware (via Alpine packages - may need expansion)
- [x] Generate filesystem.squashfs with mksquashfs

**Tiny initramfs:**
- [x] Create initramfs module (`AcornOS/src/artifact/initramfs.rs`)
- [x] Include busybox (all applets needed for boot)
- [x] Mount squashfs read-only
- [x] Mount overlay (tmpfs) for writes
- [x] switch_root to live system
- [x] Start OpenRC (via `/sbin/openrc-init`)
- [x] Init script template (`AcornOS/profile/init_tiny.template`)

**Installation tool:**
- [ ] Create extraction tool (like recstrap) - uses unsquashfs for now

**Integration:**
- [x] Update ISO builder for AcornOS layout (`AcornOS/src/artifact/iso.rs`)
- [x] Include installation tools in squashfs (Alpine packages)
- [x] Create live overlay with autologin and serial console
- [x] QEMU runner (`AcornOS/src/qemu.rs`)

---

## Current State (What's Already Working)

### Crate Structure
- [x] Cargo.toml with dependencies
- [x] src/lib.rs module structure
- [x] src/config.rs - DistroConfig implementation
- [x] src/component/mod.rs - OpenRC component stubs
- [x] src/extract.rs - APK extraction stubs
- [x] src/main.rs - CLI structure
- [x] CLAUDE.md guidelines

### distro-spec/acorn
- [x] Boot modules (12 total with .ko.gz extension)
- [x] Paths (QEMU settings, directory paths)
- [x] Services (OpenRC boot essentials)
- [x] OS constants (name, ID, ISO label)

### CLI Commands
- [x] `acornos status` - Shows configuration and build status
- [x] `acornos download` - Downloads Alpine Extended ISO and apk-tools
- [x] `acornos extract` - Extracts ISO and creates rootfs
- [x] `acornos build squashfs` - Builds filesystem.squashfs from rootfs
- [x] `acornos build` - Full build (squashfs + initramfs + ISO)
- [x] `acornos initramfs` - Builds tiny initramfs (~5MB)
- [x] `acornos iso` - Builds bootable ISO
- [x] `acornos run` - Boots ISO in QEMU

---

## Phase 1: Alpine Package Extraction

### 1.1 Package Download
- [ ] Download Alpine minirootfs as starting point
- [ ] Parse APKINDEX.tar.gz for package metadata
- [ ] Download required packages from Alpine repos
- [ ] Verify package signatures
- [ ] Cache downloaded packages

### 1.2 Package Extraction
- [ ] Extract APK files (gzipped tarballs)
- [ ] Handle package dependencies
- [ ] Merge package contents into rootfs
- [ ] Run post-install scripts (if any)

### 1.3 Alternative: Use apk-tools
- [ ] Download apk-tools-static
- [ ] Use `apk --root /mnt add <packages>`
- [ ] This handles dependencies automatically

---

## Phase 2: OpenRC Setup

### 2.1 Core OpenRC
- [ ] Copy OpenRC binary and libraries
- [ ] Create /etc/rc.conf (main config)
- [ ] Create runlevel directories:
  - /etc/runlevels/sysinit/
  - /etc/runlevels/boot/
  - /etc/runlevels/default/
  - /etc/runlevels/shutdown/
- [ ] Copy init scripts to /etc/init.d/

### 2.2 Essential Services
- [ ] **openrc-init** - PID 1
- [ ] **devfs** (sysinit) - device filesystem
- [ ] **mdev** or **eudev** (sysinit) - device manager
- [ ] **hwclock** (boot) - hardware clock
- [ ] **modules** (boot) - kernel module loading
- [ ] **sysctl** (boot) - kernel parameters
- [ ] **hostname** (boot) - set hostname
- [ ] **bootmisc** (boot) - misc boot tasks
- [ ] **networking** (boot) - network interfaces
- [ ] **syslog** (boot) - system logging
- [ ] **chronyd** or **ntpd** (default) - time sync
- [ ] **sshd** or **dropbear** (default) - SSH server
- [ ] **local** (default) - local startup scripts

### 2.3 Service Management Commands
- [ ] `rc-update add <service> <runlevel>` works
- [ ] `rc-service <service> start/stop/status` works
- [ ] `rc-status` shows service status
- [ ] `openrc <runlevel>` changes runlevel

---

## Phase 3: Device Management

### 3.1 Option A: mdev (busybox)
- [ ] Configure /etc/mdev.conf
- [ ] Coldplug existing devices at boot
- [ ] Hotplug support via uevent
- [ ] Symlinks for /dev/disk/by-uuid/, etc.

### 3.2 Option B: eudev (standalone udev fork)
- [ ] Copy eudev binaries and libraries
- [ ] Copy udev rules from /lib/udev/rules.d/
- [ ] Enable eudev service in OpenRC
- [ ] Test device detection

### 3.3 Decision Criteria
| Aspect | mdev | eudev |
|--------|------|-------|
| Size | Tiny (busybox) | ~2MB |
| Features | Basic | Full udev compat |
| Desktop apps | May have issues | Full support |
| Complexity | Simple | More complex |

**Recommendation:** Start with eudev for desktop use.

---

## Phase 4: Initramfs

### 4.1 Busybox-based initramfs
- [ ] Include busybox with all applets
- [ ] Include kernel modules for boot:
  - squashfs, overlay, loop
  - virtio_blk, virtio_scsi (VM)
  - ahci, nvme, sd_mod (hardware)
  - ext4, vfat, iso9660
- [ ] Create /init script:
  1. Mount /proc, /sys, /dev
  2. Load kernel modules
  3. Find boot device (ISO/USB)
  4. Mount squashfs read-only
  5. Mount overlay (tmpfs)
  6. switch_root to merged root
  7. exec /sbin/openrc-init

### 4.2 Init script (shell)
```sh
#!/bin/busybox sh
# AcornOS initramfs init

/bin/busybox --install -s

mount -t proc none /proc
mount -t sysfs none /sys
mount -t devtmpfs none /dev

# Load modules
modprobe squashfs
modprobe overlay
modprobe loop

# Find and mount squashfs
# ... device detection ...
mount -o loop /media/live/filesystem.squashfs /squashfs

# Create overlay
mount -t tmpfs tmpfs /overlay
mkdir -p /overlay/upper /overlay/work
mount -t overlay overlay -o lowerdir=/squashfs,upperdir=/overlay/upper,workdir=/overlay/work /newroot

# Switch root
exec switch_root /newroot /sbin/openrc-init
```

---

## Phase 5: ISO Creation

### 5.1 ISO Layout
```
acornos.iso
├── boot/
│   ├── grub/
│   │   └── grub.cfg
│   ├── vmlinuz
│   └── initramfs.img
├── live/
│   └── filesystem.squashfs
└── EFI/
    └── BOOT/
        └── BOOTX64.EFI
```

### 5.2 GRUB Configuration
```
menuentry "AcornOS Live" {
    linux /boot/vmlinuz quiet
    initrd /boot/initramfs.img
}
```

### 5.3 ISO Build
- [ ] Create directory structure
- [ ] Copy kernel, initramfs, squashfs
- [ ] Install GRUB for UEFI
- [ ] Build ISO with xorriso
- [ ] Generate SHA512 checksum

---

## What's Missing from Live Environment

These are known gaps in the live environment (squashfs):

### Critical Tools (P0)
- [x] `cryptsetup` - LUKS disk encryption
- [x] `lvm2` - Logical Volume Manager (pvcreate, vgcreate, lvcreate)
- [x] `btrfs-progs` - Btrfs filesystem tools

### Important Tools (P1)
- [x] `pciutils` (lspci) - identify PCI hardware
- [x] `usbutils` (lsusb) - identify USB devices
- [x] `dmidecode` - BIOS/DMI information
- [x] `ethtool` - NIC diagnostics and configuration
- [x] `iwd` - alternative WiFi daemon (often more reliable)
- [x] `wireless-regdb` - WiFi regulatory database

### Live Environment Config (P1)
- [x] Volatile log storage (prevent tmpfs fill) - /var/log on 64M tmpfs
- [x] do-not-suspend config (prevent sleep during install) - ACPI handler + elogind

### User Tools (Essential)
- [ ] `passwd` - interactive password setting
- [ ] `nano` or `vi` - text editor for config files
- [ ] `adduser` / `addgroup` - user/group management

### Locale & Time
- [ ] Timezone data (`/usr/share/zoneinfo/`)
- [ ] Locale support (musl locales or musl-locales package)

### Bootloader
- [ ] GRUB or syslinux for installation
- [ ] `grub-install` or `syslinux` commands

---

## 1. BOOT & INSTALLATION

### 1.0 ISO Integrity Verification (P1)
- [ ] **SHA512 checksum** - Generate `acornos-YYYY.MM.DD.iso.sha512` during build
- [ ] **GPG signature** - Sign checksum file for release verification
- [ ] Document verification process on website

### 1.1 Boot Modes
- [x] UEFI boot (GPT, ESP partition) - verified by install-tests phase 1
- [ ] BIOS/Legacy boot (MBR) - P2 (optional but nice)
- [ ] Secure Boot signed - P3 (future)

### 1.2 Boot Media
- [ ] ISO boots on real hardware (needs testing)
- [ ] ISO boots in VirtualBox (needs testing)
- [ ] ISO boots in VMware (needs testing)
- [x] ISO boots in QEMU/KVM (verified by install-tests phase 1)
- [ ] ISO boots in Hyper-V (needs testing)
- [ ] USB bootable (dd or Ventoy compatible)

### 1.3 Installation Process

**Installation Helper Scripts (like Alpine's setup-alpine):**
- [ ] **fstab generator** - Generate fstab from mounted filesystems
- [ ] **chroot helper** - Enter installed system with proper mounts

**Installation Steps:**
- [ ] Partition disk (GPT for UEFI)
- [ ] Format partitions (ext4, FAT32 for ESP)
- [ ] Mount target filesystem
- [ ] Extract squashfs to disk
- [ ] Generate fstab with UUIDs
- [ ] Set timezone (manual)
- [ ] Set locale (manual)
- [ ] Set hostname
- [ ] Set root password
- [ ] Create user account (adduser)
- [ ] Add user to wheel group
- [ ] Install bootloader (GRUB)
- [ ] Reboot into installed system

### 1.4 Post-Installation Verification
- [ ] System boots without ISO
- [ ] OpenRC is PID 1
- [ ] default runlevel reached
- [ ] No failed OpenRC services
- [ ] User can log in
- [ ] sudo/doas works
- [ ] Network is functional

---

## 2. NETWORKING

### 2.1 Network Stack Options

**Option A: ifupdown + wpa_supplicant (Alpine default)**
- [ ] /etc/network/interfaces configuration
- [ ] ifup/ifdown commands
- [ ] wpa_supplicant for WiFi

**Option B: NetworkManager (desktop friendly)**
- [ ] NetworkManager daemon
- [ ] nmcli / nmtui commands
- [ ] Automatic network detection

### 2.2 Ethernet
- [ ] DHCP client works (udhcpc or dhclient)
- [ ] Static IP configuration works
- [ ] Link detection (cable plug/unplug)
- [ ] Gigabit speeds supported
- [ ] Common drivers: e1000, e1000e, r8169

### 2.3 WiFi
- [ ] wpa_supplicant installed
- [ ] Can scan networks
- [ ] Can connect to WPA2-PSK network
- [ ] Can connect to WPA3 network
- [ ] Can connect to WPA2-Enterprise (802.1X)
- [ ] WiFi firmware: Intel (iwlwifi), Atheros, Realtek, Broadcom
- [x] **wireless-regdb** - P1: Required for legal WiFi operation
- [x] **iwd** - P1: Alternative WiFi daemon
- [x] **sof-firmware** - P1: Modern laptop sound (Intel SOF)

### 2.4 Network Tools
- [ ] `ip` - interface and routing configuration (iproute2)
- [ ] `ping` - connectivity testing (busybox or iputils)
- [ ] `ss` - socket statistics (iproute2)
- [ ] `curl` or `wget` - HTTP client
- [ ] `dig` / `nslookup` - DNS queries - P2
- [ ] `traceroute` / `tracepath` - path tracing - P2
- [x] **`ethtool`** - P1: NIC diagnostics

### 2.5 VPN Support
- [ ] OpenVPN client
- [ ] WireGuard support (kernel module + tools)
- [ ] IPsec support - *optional*

### 2.6 Remote Access
- [x] **SSH server (openssh)** - P1: Essential for remote installation
- [ ] SSH client (ssh, scp, sftp)
- [ ] Key-based authentication works

### 2.7 Firewall
- [ ] nftables OR iptables available
- [ ] awall (Alpine firewall) - *optional convenience*

---

## 3. STORAGE & FILESYSTEMS

### 3.1 Partitioning Tools
- [ ] `fdisk` - MBR/GPT partitioning (util-linux)
- [x] `parted` - GPT partitioning
- [ ] `lsblk` - list block devices
- [ ] `blkid` - show UUIDs and labels
- [ ] `wipefs` - clear filesystem signatures

### 3.2 Filesystem Support
- [ ] ext4 (e2fsprogs: mkfs.ext4, e2fsck, tune2fs)
- [ ] FAT32/vfat (dosfstools: mkfs.fat, fsck.fat) - required for ESP
- [x] XFS (xfsprogs: mkfs.xfs, xfs_repair)
- [x] **Btrfs (btrfs-progs)** - P0 CRITICAL: Popular default
- [ ] NTFS read/write (ntfs-3g) - P2: for Windows drives
- [ ] exFAT (exfatprogs) - P2: for USB drives and SD cards
- [ ] ISO9660 (kernel module)
- [ ] squashfs (kernel module + squashfs-tools)

### 3.3 LVM & RAID
- [x] **LVM2 (pvcreate, vgcreate, lvcreate)** - P0 CRITICAL
- [ ] mdadm for software RAID - P2
- [ ] dmraid for fake RAID - P3

### 3.4 Encryption
- [x] **LUKS encryption (cryptsetup)** - P0 CRITICAL
- [ ] Encrypted root partition support
- [ ] crypttab for automatic unlock

### 3.5 Mount & Automount
- [ ] `mount` / `umount` (util-linux or busybox)
- [ ] `findmnt` - show mounted filesystems
- [ ] fstab support with UUID
- [ ] automount for removable media - *optional*

### 3.6 Storage Drivers (Kernel Modules)
- [ ] SATA: ahci, ata_piix
- [ ] NVMe: nvme
- [ ] USB storage: usb-storage, uas
- [ ] SD cards: sdhci, mmc_block
- [ ] SCSI: sr_mod (CD/DVD)
- [ ] VirtIO: virtio_blk, virtio_scsi

### 3.7 Disk Health
- [x] `smartctl` (smartmontools) - SMART monitoring
- [x] `hdparm` - drive parameters
- [x] `nvme-cli` - NVMe management

---

## 4. USER MANAGEMENT

### 4.1 User Operations
- [ ] `adduser` - create users (busybox or shadow)
- [ ] `usermod` - modify users
- [ ] `deluser` - delete users
- [ ] `passwd` - change passwords
- [ ] `chpasswd` - batch password setting
- [ ] `/etc/passwd` proper format
- [ ] `/etc/shadow` proper format and permissions (0400)

### 4.2 Group Operations
- [ ] `addgroup` - create groups
- [ ] `delgroup` - delete groups
- [ ] `/etc/group` proper format
- [ ] `/etc/gshadow` proper format

### 4.3 Default Groups
- [ ] `wheel` - sudo/doas access
- [ ] `audio` - audio devices
- [ ] `video` - video devices
- [ ] `input` - input devices
- [ ] `disk` - disk devices
- [ ] `tty` - tty devices
- [ ] `users` - standard users group

### 4.4 Privilege Escalation
- [ ] `sudo` OR `doas` installed and configured
- [ ] wheel group can sudo/doas
- [ ] `su` for user switching

### 4.5 Login System
- [ ] getty on TTY1-6
- [ ] agetty autologin option
- [ ] Login shell works (ash)
- [ ] `/etc/profile` executed

---

## 5. CORE UTILITIES (busybox)

### 5.1 Busybox Applets
busybox provides most utilities. Verify these work:

- [ ] `ls`, `cp`, `mv`, `rm`, `mkdir`, `rmdir`
- [ ] `cat`, `head`, `tail`, `tee`
- [ ] `chmod`, `chown`, `chgrp`
- [ ] `ln` (symlinks and hardlinks)
- [ ] `touch`, `stat`
- [ ] `wc`, `sort`, `uniq`, `cut`
- [ ] `tr`
- [ ] `echo`, `printf`, `yes`
- [ ] `date`
- [ ] `df`, `du`
- [ ] `pwd`, `basename`, `dirname`, `realpath`
- [ ] `env`, `printenv`
- [ ] `sleep`
- [ ] `id`, `whoami`, `groups`
- [ ] `uname`
- [ ] `seq`
- [ ] `md5sum`, `sha256sum`, `sha512sum`

### 5.2 Text Processing (busybox)
- [ ] `grep`
- [ ] `sed`
- [ ] `awk`
- [ ] `diff`
- [ ] `less` (may need standalone less)
- [ ] `vi` (busybox vi)

### 5.3 File Finding (busybox)
- [ ] `find`
- [ ] `which`
- [ ] `xargs`

### 5.4 Archive Tools
- [ ] `tar` (busybox)
- [ ] `gzip`, `gunzip`
- [ ] `bzip2`, `bunzip2` (may need standalone)
- [ ] `xz`, `unxz` (may need standalone)
- [ ] `zstd` - P2
- [ ] `unzip` - P2
- [ ] `cpio`

### 5.5 Shell
- [ ] `ash` as /bin/sh (busybox)
- [ ] Command history (ash)
- [ ] Job control (bg, fg, jobs)
- [ ] `bash` - *optional, installable*

### 5.6 Standalone Utilities (not in busybox)
Some utilities need full versions:
- [ ] `less` - full pager (busybox less is limited)
- [ ] `nano` - editor (vi is in busybox)
- [ ] `file` - file type detection
- [ ] `curl` or `wget` - full HTTP client

---

## 6. SYSTEM SERVICES (OpenRC)

### 6.1 Core OpenRC
- [ ] `rc-update` - manage runlevels
- [ ] `rc-service` - manage services
- [ ] `rc-status` - show service status
- [ ] `openrc` - change runlevel

### 6.2 Essential Services (sysinit runlevel)
- [ ] `devfs` - /dev filesystem
- [ ] `mdev` or `udev-trigger` - device population
- [ ] `dmesg` - kernel message buffer

### 6.3 Boot Services (boot runlevel)
- [ ] `hwclock` - hardware clock
- [ ] `modules` - load kernel modules
- [ ] `sysctl` - kernel parameters
- [ ] `hostname` - set hostname
- [ ] `bootmisc` - misc boot tasks
- [ ] `networking` - network interfaces
- [ ] `syslog` or `syslog-ng` - logging
- [ ] `urandom` - random seed

### 6.4 Default Services (default runlevel)
- [ ] `chronyd` or `ntpd` - time sync
- [ ] `sshd` or `dropbear` - SSH server
- [ ] `local` - local startup scripts
- [ ] `acpid` - ACPI events (laptops)
- [ ] `dbus` - message bus (for desktop)

### 6.5 Logging
- [ ] syslog-ng or busybox syslogd
- [ ] Log rotation (logrotate or newsyslog)
- [ ] `/var/log/messages` exists

---

## 7. HARDWARE SUPPORT

### 7.1 CPU
- [x] **Intel microcode (intel-ucode)** - P0 CRITICAL
- [x] **AMD microcode (amd-ucode)** - P0 CRITICAL
- [ ] CPU frequency scaling (cpupower) - P2
- [ ] Temperature monitoring (lm_sensors) - P2

### 7.2 Memory
- [ ] Swap partition/file support
- [ ] zram/zswap - *optional*
- [ ] `free` - memory stats
- [ ] `/proc/meminfo` readable

### 7.3 PCI/USB Detection
- [x] **`lspci` (pciutils)** - P1
- [x] **`lsusb` (usbutils)** - P1
- [ ] `lshw` - *optional*
- [x] **`dmidecode`** - P1

### 7.4 Input Devices
- [ ] Keyboard works (all layouts via loadkeys)
- [ ] Mouse works (PS/2 and USB)
- [ ] Touchpad works (libinput)
- [ ] Keymaps in /usr/share/kbd/keymaps/

### 7.5 Display (Framebuffer/Console)
- [ ] Console fonts (terminus-font or similar)
- [ ] `setfont` - change console font
- [ ] Virtual consoles (Ctrl+Alt+F1-F6)

### 7.6 Audio (Console/Headless)
- [ ] ALSA utilities (alsa-utils) - *optional*
- [ ] `amixer`, `alsamixer` - *optional*

### 7.7 Graphics (Optional - for Desktop)
- [ ] Intel graphics (i915)
- [ ] AMD graphics (amdgpu)
- [ ] NVIDIA (nouveau or proprietary)
- [ ] VirtualBox Guest Additions
- [ ] VMware SVGA driver
- [ ] QXL for QEMU/KVM

### 7.8 Bluetooth - *optional*
- [ ] BlueZ stack
- [ ] `bluetoothctl`
- [ ] Firmware for common adapters

---

## 8. PACKAGE MANAGEMENT

### 8.1 Alpine apk (Native)
- [ ] `apk add <package>` - install packages
- [ ] `apk del <package>` - remove packages
- [ ] `apk update` - update index
- [ ] `apk upgrade` - upgrade packages
- [ ] `apk search <query>` - search packages
- [ ] `apk info <package>` - show info
- [ ] Repository configuration (/etc/apk/repositories)

### 8.2 Recipe Package Manager (Optional)
- [ ] `recipe search <package>` - search packages
- [ ] `recipe install <package>` - install packages
- [ ] `recipe remove <package>` - remove packages
- [ ] Recipe repos can supplement apk

---

## 9. DEVELOPMENT (Optional)

### 9.1 Build Tools
- [ ] `gcc` / `clang` - installable
- [ ] `make` - installable
- [ ] `cmake` - installable
- [ ] `pkg-config`
- [ ] `musl-dev` - musl headers

### 9.2 Version Control
- [ ] `git` - installable

### 9.3 Scripting
- [ ] Python 3 - installable
- [ ] Perl - installable
- [ ] Node.js - installable

---

## 10. VIRTUALIZATION SUPPORT

### 10.1 Guest Additions
- [ ] QEMU guest agent (qemu-guest-agent)
- [ ] VirtualBox Guest Additions (virtualbox-guest-additions)
- [ ] VMware Tools (open-vm-tools)
- [ ] Hyper-V daemons

### 10.2 VirtIO Drivers
- [ ] virtio_blk - block devices
- [ ] virtio_net - networking
- [ ] virtio_scsi - SCSI
- [ ] virtio_console - console
- [ ] virtio_balloon - memory ballooning
- [ ] virtio_gpu - graphics

---

## 11. SECURITY

### 11.1 Basic Security
- [ ] `/etc/shadow` permissions 0400
- [ ] Root account locked by default? (configurable)
- [ ] Password hashing (SHA-512)
- [ ] Failed login delays

### 11.2 SSH Security
- [ ] Root login disabled by default
- [ ] Key-based auth preferred
- [ ] SSH host keys generated

### 11.3 Firewall
- [ ] nftables/iptables available
- [ ] awall (Alpine firewall) - *optional*

---

## 12. RECOVERY & DIAGNOSTICS

### 12.1 Recovery Tools
- [ ] Single-user mode (init=/bin/sh)
- [ ] Live ISO can rescue installed system
- [ ] `fsck` for all supported filesystems
- [ ] `testdisk` - *optional*
- [ ] `ddrescue` - *optional*

### 12.2 Diagnostic Tools
- [ ] `dmesg` - kernel messages
- [ ] `logread` - syslog messages (busybox)
- [ ] `rc-status` - failed services
- [ ] `/var/log/` directory structure

### 12.3 Performance Tools
- [ ] `top` / `htop`
- [ ] `ps` - process listing
- [ ] `kill`, `killall`, `pkill`
- [ ] `nice`, `renice`
- [ ] `iostat`, `vmstat` - *optional*

---

## 13. LOCALIZATION

### 13.1 Locale
- [ ] UTF-8 support (en_US.UTF-8 default)
- [ ] musl-locales package (if needed)
- [ ] `/etc/locale.conf` or equivalent

### 13.2 Timezone
- [ ] tzdata installed
- [ ] `/etc/localtime` symlink
- [ ] `setup-timezone` or manual

### 13.3 Keyboard
- [ ] US layout default
- [ ] Other layouts available
- [ ] `/etc/conf.d/keymaps` or equivalent

### 13.4 Console Fonts
- [ ] Readable default font
- [ ] Unicode support

---

## 14. DOCUMENTATION

### 14.1 Man Pages
- [ ] `man` command works (mandoc)
- [ ] Core command man pages included

### 14.2 Online Documentation
- [ ] Installation guide on website
- [ ] Wiki or knowledge base

---

## 15. MUSL COMPATIBILITY

### 15.1 Known musl Issues
Some software has glibc-specific assumptions:

- [ ] **DNS resolution** - musl's resolver is simpler
- [ ] **Locale** - musl has limited locale support
- [ ] **Thread stack size** - musl defaults differ
- [ ] **Name Service Switch** - musl doesn't support NSS

### 15.2 Workarounds
- [ ] Document musl-incompatible packages
- [ ] Provide gcompat (glibc compat layer) for problematic apps
- [ ] Test desktop apps for musl compatibility

### 15.3 Testing Matrix

| Application | musl Status | Notes |
|-------------|-------------|-------|
| Firefox | Works (Alpine build) | |
| Chromium | Works (Alpine build) | |
| GNOME | Works | Needs testing |
| Sway | Works | |
| Docker | Works | |
| Node.js | Works | |
| Python | Works | |
| Java/JVM | Works (OpenJDK) | |

---

## PRIORITY LEVELS

### P0 - Must Have (Blocking Daily Driver)
- [ ] Boot and installation works (UEFI)
- [ ] Network (Ethernet + DHCP)
- [ ] WiFi support (wpa_supplicant + firmware)
- [ ] User management + sudo/doas
- [ ] Core utilities (busybox)
- [ ] OpenRC working
- [x] Device management (eudev or mdev) - eudev added
- [x] **Intel/AMD microcode** - CPU security/stability
- [x] **cryptsetup (LUKS)** - disk encryption
- [x] **lvm2** - Logical Volume Manager
- [x] **btrfs-progs** - Btrfs filesystem support

### P1 - Should Have (Alpine parity)
- [x] Volatile log storage (prevent tmpfs fill)
- [x] do-not-suspend config
- [x] SSH server (openssh)
- [x] Hardware probing: lspci, lsusb, dmidecode
- [x] ethtool (NIC diagnostics)
- [x] iwd (alternative WiFi)
- [x] wireless-regdb (regulatory compliance)
- [x] sof-firmware (Intel laptop sound)
- [ ] ISO SHA512 checksum generation
- [ ] Man pages

### P2 - Nice to Have
- BIOS/Legacy boot
- VPN support (WireGuard, OpenVPN)
- VM guest tools
- Recovery tools (testdisk, ddrescue)
- XFS, exFAT, NTFS support

### P3 - Future
- Secure Boot signing
- Full accessibility
- Guided installer

---

## TEST MATRIX

| Category | Items | Tested | Notes |
|----------|-------|--------|-------|
| Boot | 10 | Partial | UEFI+QEMU verified via install-tests phase 1 |
| Network | 25+ | No | |
| Storage | 20+ | No | |
| Users | 15+ | No | |
| Utilities | 40+ | No | busybox applets |
| OpenRC | 15+ | Partial | Boots to default runlevel (install-tests) |
| Hardware | 25+ | No | |
| Packages | 5 | No | |
| Security | 10+ | No | |
| Recovery | 10+ | No | |
| Locale | 6 | No | |

---

## ALPINE PACKAGE LIST (Reference)

These Alpine packages provide the functionality needed:

### Base System
- `alpine-base` - meta package
- `busybox` - coreutils replacement
- `busybox-initscripts` - OpenRC scripts
- `openrc` - init system
- `musl` - C library

### Boot
- `linux-lts` - kernel
- `linux-firmware` - firmware
- `grub` or `syslinux` - bootloader
- `efibootmgr` - UEFI

### Storage
- `e2fsprogs` - ext4
- `dosfstools` - FAT32
- `btrfs-progs` - Btrfs
- `lvm2` - LVM
- `cryptsetup` - LUKS
- `parted` - partitioning
- `util-linux` - fdisk, lsblk, blkid

### Network
- `networkmanager` OR `ifupdown`
- `wpa_supplicant` - WiFi
- `iproute2` - ip command
- `dhcpcd` or `udhcpc` - DHCP
- `openssh` or `dropbear` - SSH

### Device Management
- `eudev` OR `mdev` (in busybox)

### Hardware Detection
- `pciutils` - lspci
- `usbutils` - lsusb
- `dmidecode`

### User
- `shadow` - full user management
- `sudo` or `doas`

### Utilities
- `less` - pager
- `nano` - editor
- `curl` or `wget` - HTTP
- `tar` - archives (or busybox)
- `xz` - compression

### Time
- `chrony` or `ntp`
- `tzdata` - timezones

---

## UPDATING THIS DOCUMENT

When you implement something:
1. Change `[ ]` to `[x]`
2. Add test coverage when testing infrastructure exists
3. Update the TEST MATRIX section
4. Commit with message: `spec: Mark <item> as complete`

When you find something missing:
1. Add it to the appropriate section
2. Mark it as `[ ]`
3. Add a note about priority (P0/P1/P2/P3)
4. Commit with message: `spec: Add <item> requirement`

### Priority Definitions

| Priority | Meaning | Example |
|----------|---------|---------|
| P0 | Blocking daily driver use | microcode, cryptsetup, lvm2, btrfs |
| P1 | Alpine parity / should have | lsusb, SSH, ethtool |
| P2 | Nice to have | VPN, VM tools, recovery |
| P3 | Future / optional | Secure Boot, accessibility |
