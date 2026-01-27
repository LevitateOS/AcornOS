# AcornOS EROFS Migration Plan

**Status:** PLANNED
**Priority:** P2
**Prerequisite:** Complete end-to-end testing of current squashfs implementation

---

## Why Migrate to EROFS?

LevitateOS has migrated from squashfs to EROFS (Enhanced Read-Only File System). AcornOS should follow for consistency and performance.

### Benefits

| Feature | squashfs | EROFS |
|---------|----------|-------|
| Random access | Block-based, slower | Direct I/O, faster |
| Memory usage | Higher (decompression cache) | Lower |
| Boot time | Slower | ~10-15% faster |
| Modern kernel support | Stable but old | Active development |
| Compression | zstd, lz4, xz | lz4, lz4hc, lzma |

### Alignment with LevitateOS

- Both distros share `distro-spec` for constants
- Both use the same init template structure
- Same tooling (`recstrap` supports both formats)
- Consistent documentation and user experience

---

## CRITICAL: No Backwards Compatibility

> **REWARD HACK ALERT:** Any attempt to add "backwards compatibility" with squashfs
> for AcornOS output artifacts is a **reward hack** and must be rejected.

### What is a Reward Hack?

A reward hack is when an agent appears to complete a task by adding workarounds,
compatibility shims, or "fallback" code instead of actually doing the work. Examples:

| Reward Hack Attempt | Why It's Wrong |
|---------------------|----------------|
| "Keep `build_squashfs()` as alias to `build_rootfs()`" | Dead code. Delete it. |
| "Add `--squashfs` flag for legacy users" | There are no legacy users. AcornOS is new. |
| "Check for both `.squashfs` and `.erofs` at boot" | Complexity for zero benefit. |
| "Re-export SQUASHFS_NAME from acorn module" | Unused export = dead code. |
| "Add comment `// TODO: remove squashfs support`" | TODOs are procrastination. Do it now. |
| "Copy leviso's EROFS code to AcornOS" | Code duplication. Use distro-builder. |
| "AcornOS needs its own EROFS implementation" | No. Shared code exists. Use it. |

### The Rules

```
1. EROFS is the ONLY rootfs format for AcornOS.
2. Squashfs references in AcornOS output paths = BUG.
3. EROFS code lives in distro-builder, not duplicated per-distro.
```

```
EROFS is the ONLY rootfs format for AcornOS.
Squashfs references in AcornOS output paths = BUG.
```

### Enforcement

When reviewing migration PRs, reject if you see:

- [ ] Alias functions (`build_squashfs` calling `build_rootfs`)
- [ ] Dual-format detection in init scripts
- [ ] SQUASHFS_* constants exported from `acorn/` module
- [ ] "Fallback" logic checking for `.squashfs` files
- [ ] Comments saying "keep for compatibility"
- [ ] `mkfs.erofs` invocation duplicated in AcornOS (should use distro-builder)
- [ ] `Cmd::new("mkfs.erofs")` anywhere except distro-builder/src/artifact/rootfs.rs

---

## What to Keep (Legitimate squashfs Uses)

These are the ONLY acceptable squashfs references in the codebase:

| Reference | Why Keep |
|-----------|----------|
| `unsquashfs` tool | Extracts Alpine's modloop (which IS squashfs) |
| `squashfs-tools` package | Provides unsquashfs for Alpine extraction |
| `CONFIG_SQUASHFS` in kconfig | Mount Alpine's squashfs during extraction |
| `kernel/fs/squashfs/squashfs` module | For mounting source media |

**Note:** These are for reading UPSTREAM squashfs (Alpine's packages), NOT for
creating AcornOS artifacts. AcornOS outputs EROFS only.

---

## Architecture: Shared Code in distro-builder

> **CRITICAL:** Do NOT duplicate EROFS code between leviso and AcornOS.
> The EROFS building logic belongs in `distro-builder` crate.

### Current State

```
distro-builder/src/artifact/rootfs.rs
├── RootfsOptions struct        ✓ Defined
├── build_rootfs() function     ✗ UNIMPLEMENTED (placeholder)
└── build_squashfs() function   ✗ UNIMPLEMENTED (to be deleted)

leviso/src/artifact/rootfs.rs
└── create_erofs_internal()     ✓ Working EROFS implementation

AcornOS/src/artifact/squashfs.rs
└── create_squashfs_internal()  ✓ Working squashfs implementation
```

### Target State

```
distro-builder/src/artifact/rootfs.rs
├── RootfsOptions struct        ✓ Keep
├── create_erofs() function     ✓ IMPLEMENT (extract from leviso)
└── build_squashfs()            ✗ DELETE (reward hack)

leviso/src/artifact/rootfs.rs
└── Uses distro_builder::artifact::rootfs::create_erofs()

AcornOS/src/artifact/rootfs.rs
└── Uses distro_builder::artifact::rootfs::create_erofs()
```

---

## Files to Modify

### Phase 0: Extract EROFS to distro-builder (PREREQUISITE)

Before AcornOS migration, extract leviso's EROFS code to shared crate.

#### distro-builder/src/artifact/rootfs.rs

Replace the unimplemented placeholder with actual code from leviso:

```rust
use crate::process::Cmd;

/// Create an EROFS image from a directory.
///
/// This is the shared implementation used by both LevitateOS and AcornOS.
pub fn create_erofs(
    source_dir: &Path,
    output: &Path,
    compression: &str,
    compression_level: u8,
    chunk_size: u32,
) -> Result<()> {
    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let compression_arg = format!("{},{}", compression, compression_level);

    // IMPORTANT: mkfs.erofs argument order is OUTPUT SOURCE
    Cmd::new("mkfs.erofs")
        .args(["-z", &compression_arg])
        .args(["-C", &chunk_size.to_string()])
        .arg("--all-root")
        .arg("-T0")  // Reproducible builds
        .arg_path(output)
        .arg_path(source_dir)
        .error_msg("mkfs.erofs failed. Install erofs-utils: sudo dnf install erofs-utils")
        .run_interactive()?;

    Ok(())
}

// DELETE these - they are reward hacks:
// pub fn build_squashfs(...) { ... }
// pub enum RootfsFormat { Erofs, Squashfs }  // Remove Squashfs variant
```

#### Update leviso to use shared code

```rust
// leviso/src/artifact/rootfs.rs

use distro_builder::artifact::rootfs::create_erofs;
use distro_spec::levitate::{EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, EROFS_CHUNK_SIZE};

fn create_erofs_internal(staging: &Path, output: &Path) -> Result<()> {
    // Use shared implementation
    create_erofs(
        staging,
        output,
        EROFS_COMPRESSION,
        EROFS_COMPRESSION_LEVEL,
        EROFS_CHUNK_SIZE,
    )
}
```

---

### Phase 1: distro-spec constants

#### distro-spec/src/acorn/paths.rs

```rust
// EROFS constants (same structure as levitate/)
pub const EROFS_NAME: &str = "filesystem.erofs";
pub const EROFS_CDROM_PATH: &str = "/live/filesystem.erofs";
pub const EROFS_COMPRESSION: &str = "lz4hc";
pub const EROFS_COMPRESSION_LEVEL: u8 = 9;
pub const EROFS_CHUNK_SIZE: u32 = 1048576;  // 1MB

// Generic aliases
pub const ROOTFS_NAME: &str = EROFS_NAME;
pub const ROOTFS_CDROM_PATH: &str = EROFS_CDROM_PATH;
pub const ROOTFS_TYPE: &str = "erofs";
```

#### distro-spec/src/acorn/mod.rs

```rust
pub use paths::{
    // EROFS only - NO squashfs exports
    EROFS_NAME, EROFS_CDROM_PATH, EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, EROFS_CHUNK_SIZE,
    ROOTFS_NAME, ROOTFS_CDROM_PATH, ROOTFS_TYPE,
};
```

---

### Phase 2: AcornOS artifact module

#### AcornOS/src/artifact/squashfs.rs -> DELETE

Do not rename. Delete the entire file.

#### AcornOS/src/artifact/rootfs.rs -> CREATE (thin wrapper)

```rust
//! Rootfs builder - creates the AcornOS EROFS system image.

use anyhow::{bail, Result};
use std::fs;
use std::path::Path;

use distro_builder::artifact::rootfs::create_erofs;
use distro_spec::acorn::{
    EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL, EROFS_CHUNK_SIZE, ROOTFS_NAME,
};

use crate::component::{build_system, BuildContext};
use crate::extract::ExtractPaths;

/// Build the EROFS rootfs using the component system.
pub fn build_rootfs(base_dir: &Path) -> Result<()> {
    println!("=== Building AcornOS EROFS System Image ===\n");

    let paths = ExtractPaths::new(base_dir);
    let output_dir = base_dir.join("output");
    let staging = output_dir.join("rootfs-staging");
    fs::create_dir_all(&output_dir)?;

    if !paths.rootfs.exists() || !paths.rootfs.join("bin").exists() {
        bail!(
            "Rootfs not found at {}.\nRun 'acornos extract' first.",
            paths.rootfs.display()
        );
    }

    let ctx = BuildContext::new(base_dir, &staging)?;
    build_system(&ctx)?;

    let final_output = output_dir.join(ROOTFS_NAME);

    println!("\nCreating EROFS from staging...");
    println!("  Compression: {},{}", EROFS_COMPRESSION, EROFS_COMPRESSION_LEVEL);

    // Use shared distro-builder implementation
    create_erofs(
        &staging,
        &final_output,
        EROFS_COMPRESSION,
        EROFS_COMPRESSION_LEVEL,
        EROFS_CHUNK_SIZE,
    )?;

    println!("\n=== EROFS Build Complete ===");
    println!("  Output: {}", final_output.display());

    Ok(())
}
```

### 4. AcornOS/src/artifact/mod.rs

Update exports:

```rust
// OLD
pub mod squashfs;
pub use squashfs::build_squashfs;
pub use iso::create_squashfs_iso;

// NEW
pub mod rootfs;  // renamed from squashfs
pub use rootfs::build_rootfs;
pub use iso::create_iso;
```

### 5. AcornOS/src/artifact/iso.rs

Rename function and update paths:

```rust
// OLD
pub fn create_squashfs_iso(base_dir: &Path) -> Result<()>

// NEW
pub fn create_iso(base_dir: &Path) -> Result<()>
```

Update file paths inside:

```rust
// OLD
let squashfs = output_dir.join(SQUASHFS_NAME);
fs::copy(&squashfs, iso_staging.join("live").join(SQUASHFS_NAME))?;

// NEW
let rootfs = output_dir.join(ROOTFS_NAME);
fs::copy(&rootfs, iso_staging.join("live").join(ROOTFS_NAME))?;
```

### 6. AcornOS/src/rebuild.rs

Rename functions and update constants:

```rust
// OLD
pub fn squashfs_needs_rebuild(base_dir: &Path) -> bool
pub fn cache_squashfs_hash(base_dir: &Path)

// NEW
pub fn rootfs_needs_rebuild(base_dir: &Path) -> bool
pub fn cache_rootfs_hash(base_dir: &Path)
```

Update hash file names:

```rust
// OLD
let hash_file = base_dir.join("output/.squashfs-inputs.hash");

// NEW
let hash_file = base_dir.join("output/.rootfs-inputs.hash");
```

### 7. AcornOS/src/main.rs

Update CLI commands and function calls:

```rust
// OLD
enum BuildArtifact {
    Squashfs,
}

fn cmd_build_squashfs() -> Result<()>

// Check for squashfs existence
let squashfs = base_dir.join("output/filesystem.squashfs");
if !squashfs.exists() { ... }

// NEW
enum BuildArtifact {
    Rootfs,
}

fn cmd_build_rootfs() -> Result<()>

// Use constant
use distro_spec::acorn::ROOTFS_NAME;
let rootfs = base_dir.join("output").join(ROOTFS_NAME);
if !rootfs.exists() { ... }
```

### 8. AcornOS/profile/init_tiny.template

Rename mount point and update filesystem type:

```bash
# OLD
busybox mkdir -p /squashfs /live-overlay /overlay /overlay/upper /overlay/work /newroot
busybox mount -t squashfs -o ro /dev/loop0 /squashfs
-o lowerdir=/live-overlay:/squashfs,upperdir=/overlay/upper,workdir=/overlay/work

# NEW
busybox mkdir -p /rootfs /live-overlay /overlay /overlay/upper /overlay/work /newroot
busybox mount -t erofs -o ro /dev/loop0 /rootfs
-o lowerdir=/live-overlay:/rootfs,upperdir=/overlay/upper,workdir=/overlay/work
```

Update template variables:

```bash
# OLD
{{SQUASHFS_PATH}}

# NEW
{{ROOTFS_PATH}}
```

Update comments throughout to say "EROFS" instead of "squashfs".

### 9. AcornOS/src/artifact/initramfs.rs

Update template variable substitution:

```rust
// OLD
.replace("{{SQUASHFS_PATH}}", SQUASHFS_PATH)

// NEW
.replace("{{ROOTFS_PATH}}", ROOTFS_CDROM_PATH)
```

### 10. AcornOS/src/qemu.rs

Update error patterns:

```rust
// OLD
"SQUASHFS error",

// NEW
"EROFS error",
```

### 11. AcornOS/CLAUDE.md

Update documentation:

```markdown
# OLD
- Squashfs builder

# NEW
- EROFS rootfs builder
```

Update commands section:

```markdown
# OLD
acornos build squashfs   # Build only squashfs

# NEW
acornos build rootfs     # Build only rootfs (EROFS)
```

### 12. AcornOS/src/component/custom/live.rs

Update any squashfs path references to use ROOTFS constants.

---

## Order of Operations

### Phase 0: Extract to distro-builder (BLOCKS everything else)

1. **Extract `create_erofs()` to distro-builder** - Move from leviso
2. **Delete `build_squashfs()` from distro-builder** - It's a reward hack placeholder
3. **Update leviso to use shared code** - Thin wrapper only
4. **Verify leviso still builds** - `cd leviso && cargo build && cargo test`

### Phase 1: distro-spec constants

5. **Add EROFS constants to `distro-spec/src/acorn/paths.rs`**
6. **Export from `distro-spec/src/acorn/mod.rs`** - NO squashfs exports

### Phase 2: AcornOS migration

7. **DELETE `AcornOS/src/artifact/squashfs.rs`** - Not rename, DELETE
8. **CREATE `AcornOS/src/artifact/rootfs.rs`** - Thin wrapper using distro-builder
9. **Update `AcornOS/src/artifact/mod.rs`** - Export build_rootfs
10. **Update `AcornOS/src/rebuild.rs`** - Rename functions and paths
11. **Update `AcornOS/src/main.rs`** - CLI commands
12. **Update init template** - Mount point and filesystem type
13. **Update documentation**

### Phase 3: Verification

14. **Build test** - `cd AcornOS && cargo build`
15. **Unit tests** - `cargo test`
16. **Full build** - `cargo run -- build`
17. **Boot test** - `cargo run -- run`
18. **Run reward hack detection script**

---

## Verification Steps

```bash
# 1. Build should work
cd AcornOS && cargo build

# 2. No squashfs in AcornOS output paths (except Alpine extraction)
grep -rn "filesystem.squashfs" AcornOS/src/
# Should return ZERO matches

# 3. No SQUASHFS exports from acorn module
grep -rn "SQUASHFS" distro-spec/src/acorn/
# Should return ZERO matches (squashfs constants stay in shared/ only)

# 4. No alias functions (reward hack detection)
grep -rn "build_squashfs\|create_squashfs" AcornOS/src/
# Should return ZERO matches

# 5. Tests pass
cargo test

# 6. ISO builds with EROFS
cargo run -- build

# 7. Boot verification
cargo run -- run

# 8. Verify EROFS is actually used (in QEMU console)
mount | grep erofs
# Should show: /dev/loop0 on /rootfs type erofs

# 9. Verify NO squashfs mount for rootfs
mount | grep squashfs
# Should show NOTHING (or only Alpine modloop if applicable)
```

### Reward Hack Detection Script

Run this after migration to catch any compatibility shims or code duplication:

```bash
#!/bin/bash
# detect-reward-hacks.sh

ERRORS=0

echo "Checking for reward hack attempts..."

# Check for alias functions
if grep -rq "pub fn build_squashfs" AcornOS/src/; then
    echo "FAIL: build_squashfs alias function exists"
    ERRORS=$((ERRORS + 1))
fi

# Check for dual-format detection
if grep -rq "squashfs.*||.*erofs\|erofs.*||.*squashfs" AcornOS/src/; then
    echo "FAIL: Dual-format detection found"
    ERRORS=$((ERRORS + 1))
fi

# Check for squashfs exports from acorn module
if grep -rq "SQUASHFS" distro-spec/src/acorn/mod.rs; then
    echo "FAIL: SQUASHFS constants exported from acorn module"
    ERRORS=$((ERRORS + 1))
fi

# Check for "fallback" comments
if grep -riq "fallback.*squashfs\|squashfs.*fallback\|compat.*squashfs" AcornOS/; then
    echo "FAIL: Fallback/compat comments found"
    ERRORS=$((ERRORS + 1))
fi

# Check for CODE DUPLICATION: mkfs.erofs should only be in distro-builder
EROFS_CALLS=$(grep -r "mkfs.erofs" --include="*.rs" | grep -v "distro-builder/src/artifact/rootfs.rs" | grep -v "// " | wc -l)
if [ "$EROFS_CALLS" -gt 0 ]; then
    echo "FAIL: mkfs.erofs invocation outside distro-builder (code duplication)"
    grep -r "mkfs.erofs" --include="*.rs" | grep -v "distro-builder/src/artifact/rootfs.rs" | grep -v "// "
    ERRORS=$((ERRORS + 1))
fi

# Check that AcornOS uses distro_builder::artifact::rootfs
if ! grep -rq "distro_builder::artifact::rootfs\|use distro_builder.*rootfs" AcornOS/src/artifact/rootfs.rs 2>/dev/null; then
    echo "FAIL: AcornOS doesn't use shared distro_builder::artifact::rootfs"
    ERRORS=$((ERRORS + 1))
fi

if [ $ERRORS -eq 0 ]; then
    echo "PASS: No reward hacks detected"
else
    echo "FAIL: $ERRORS reward hack(s) detected"
    exit 1
fi
```

---

## Host Tool Requirements

The migration requires `erofs-utils` on the build host:

```bash
# Fedora/RHEL
sudo dnf install erofs-utils

# Ubuntu/Debian
sudo apt install erofs-utils

# Arch
sudo pacman -S erofs-utils
```

Minimum version: erofs-utils 1.5+ (for lz4hc compression)

---

## Kernel Requirements

AcornOS's kconfig must include:

```
CONFIG_EROFS_FS=y
CONFIG_EROFS_FS_XATTR=y
CONFIG_EROFS_FS_POSIX_ACL=y
CONFIG_EROFS_FS_ZIP=y
CONFIG_EROFS_FS_ZIP_LZMA=y
```

**Note:** Keep `CONFIG_SQUASHFS=y` for mounting Alpine's source media.

---

## Rollback Plan (Emergency Only)

> **WARNING:** Rollback means "git revert the migration commit", NOT "add
> squashfs as a fallback option". If you're tempted to keep both formats
> "just in case", re-read the Reward Hack section above.

If EROFS has a blocking bug that prevents boot:

1. `git revert <migration-commit>` - Full revert, not partial
2. File a bug against erofs-utils or kernel
3. Wait for upstream fix
4. Re-apply migration when fixed

**NOT acceptable:**
- "Keep squashfs as fallback until EROFS is stable"
- "Add --use-squashfs flag for debugging"
- "Check for both formats at boot time"

---

## Timeline

| Phase | Task | Estimated Effort |
|-------|------|------------------|
| 0a | Complete squashfs e2e testing | Prerequisite |
| 0b | Extract `create_erofs()` to distro-builder | 1-2 hours |
| 0c | Update leviso to use shared code | 30 min |
| 1 | distro-spec acorn constants | 30 min |
| 2 | AcornOS artifact module (delete+create) | 1 hour |
| 2 | Update rebuild.rs, main.rs, iso.rs | 1 hour |
| 2 | Update init template | 30 min |
| 3 | Testing and fixes | 2-4 hours |
| 3 | Documentation | 30 min |

**Total:** ~8-10 hours of work

**Note:** Phase 0 (distro-builder extraction) benefits both distros and
should be done first. It ensures no code duplication.

---

## References

- LevitateOS EROFS migration: `.teams/TEAM_121_erofs-migration.md`
- EROFS documentation: https://erofs.docs.kernel.org/
- erofs-utils: https://git.kernel.org/pub/scm/linux/kernel/git/xiang/erofs-utils.git
