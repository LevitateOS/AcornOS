// Linux kernel - dependency recipe for AcornOS
//
// Provides: Built Linux kernel for AcornOS
// Output: vmlinuz + modules in output/staging
//
// NOTE: AcornOS shares the kernel with LevitateOS. If leviso has already
// built the kernel, we steal it instead of rebuilding (saves ~1 hour).
//
// Phases:
// - acquire: Get source from submodule or check leviso's build
// - build: Compile kernel OR steal from leviso
// - install: Copy vmlinuz and modules to staging
//
// Source priority:
// 1. Steal from leviso/output/kernel-build (if exists)
// 2. Git submodule at MONOREPO/linux
// 3. Git clone from github.com/LevitateOS/linux (fallback)

let ctx = #{
    description: "Linux kernel (shared with LevitateOS)",
    name: "linux",
    git_url: "https://github.com/LevitateOS/linux.git",
    // Paths filled in by phases
    source_path: "",
    submodule_path: "",
    leviso_kernel_path: "",
    build_dir: "",
    kernel_version: "",
    stolen: false,
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    // BUILD_DIR is AcornOS/downloads
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);

    // Check for leviso's pre-built kernel (theft mode)
    let leviso_kernel = join_path(monorepo, "leviso/output/kernel-build");
    let leviso_bzimage = join_path(leviso_kernel, "arch/x86/boot/bzImage");
    if is_file(leviso_bzimage) {
        ctx.leviso_kernel_path = leviso_kernel;
        ctx.stolen = true;
        return ctx;
    }

    // Check for submodule
    let submodule = join_path(monorepo, "linux");
    ctx.submodule_path = submodule;
    if is_file(join_path(submodule, "Makefile")) {
        ctx.source_path = submodule;
        return ctx;
    }

    // Check for downloaded source
    let downloaded = join_path(BUILD_DIR, "linux");
    if is_file(join_path(downloaded, "Makefile")) {
        ctx.source_path = downloaded;
        return ctx;
    }

    throw "linux source not acquired";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);

    // Check for leviso's pre-built kernel first (theft mode)
    let leviso_kernel = join_path(monorepo, "leviso/output/kernel-build");
    let leviso_bzimage = join_path(leviso_kernel, "arch/x86/boot/bzImage");
    if is_file(leviso_bzimage) {
        log("Found pre-built kernel from LevitateOS - will steal it!");
        ctx.leviso_kernel_path = leviso_kernel;
        ctx.stolen = true;
        return ctx;
    }

    let submodule = join_path(monorepo, "linux");
    ctx.submodule_path = submodule;

    // Try to init submodule
    let git_file = join_path(submodule, ".git");
    if exists(git_file) {
        log("Initializing linux submodule...");
        let status = shell_status("git -C " + monorepo + " submodule update --init --depth=1 linux");
        if status == 0 {
            if is_file(join_path(submodule, "Makefile")) {
                ctx.source_path = submodule;
                log("Linux submodule initialized at " + submodule);
                return ctx;
            }
        }
        warn("Submodule init failed, falling back to git clone");
    }

    // Fallback: clone from LevitateOS fork
    let clone_dir = join_path(BUILD_DIR, "linux");
    if !is_dir(clone_dir) {
        log("Cloning Linux kernel from " + ctx.git_url);
        shell("git clone --depth=1 " + ctx.git_url + " " + clone_dir);
    }

    if is_file(join_path(clone_dir, "Makefile")) {
        ctx.source_path = clone_dir;
        return ctx;
    }

    throw "Failed to acquire linux source";
}

// === BUILD ===

fn is_built(ctx) {
    // If we're stealing from leviso, it's already built
    if ctx.stolen {
        return ctx;
    }

    let acornos_dir = dirname(BUILD_DIR);
    let build_dir = join_path(acornos_dir, "output/kernel-build");
    let bzimage = join_path(build_dir, "arch/x86/boot/bzImage");

    if !is_file(bzimage) {
        throw "kernel not built";
    }

    ctx.build_dir = build_dir;
    ctx
}

fn build(ctx) {
    // If stealing from leviso, just symlink
    if ctx.stolen {
        log("STEALING KERNEL FROM LEVITATEOS");
        log("Skipping kernel build - using leviso's pre-built kernel");

        let acornos_dir = dirname(BUILD_DIR);
        let our_build = join_path(acornos_dir, "output/kernel-build");

        // Remove existing if present
        if exists(our_build) {
            rm(our_build);
        }

        // Create symlink to leviso's build
        mkdir(join_path(acornos_dir, "output"));
        shell("ln -s " + ctx.leviso_kernel_path + " " + our_build);
        log("Created symlink: output/kernel-build -> " + ctx.leviso_kernel_path);

        ctx.build_dir = our_build;
        return ctx;
    }

    // Build kernel from source
    let acornos_dir = dirname(BUILD_DIR);
    let kconfig = join_path(acornos_dir, "kconfig");
    let build_dir = join_path(acornos_dir, "output/kernel-build");

    if !is_file(kconfig) {
        throw "kconfig not found at " + kconfig;
    }

    mkdir(build_dir);

    log("Building Linux kernel...");
    log("  Source: " + ctx.source_path);
    log("  Config: " + kconfig);
    log("  Output: " + build_dir);

    // Copy config
    shell("cp " + kconfig + " " + join_path(build_dir, ".config"));

    // Build kernel
    let jobs = shell_output("nproc");
    shell("make -C " + ctx.source_path + " O=" + build_dir + " olddefconfig");
    shell("make -C " + ctx.source_path + " O=" + build_dir + " -j" + trim(jobs));

    ctx.build_dir = build_dir;
    log("Kernel build complete");
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let vmlinuz = join_path(staging, "boot/vmlinuz");
    let modules = join_path(staging, "lib/modules");

    if !is_file(vmlinuz) {
        throw "vmlinuz not installed";
    }
    if !is_dir(modules) {
        throw "modules not installed";
    }

    ctx
}

fn install(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let build_dir = if ctx.stolen { ctx.leviso_kernel_path } else { ctx.build_dir };

    // Determine source path for kernel source tree
    let source_path = if ctx.source_path != "" {
        ctx.source_path
    } else {
        // If stolen, use the monorepo linux submodule
        let monorepo = dirname(acornos_dir);
        join_path(monorepo, "linux")
    };

    log("Installing kernel to staging...");

    // Create directories
    mkdir(join_path(staging, "boot"));
    mkdir(join_path(staging, "lib/modules"));

    // Copy vmlinuz
    let bzimage = join_path(build_dir, "arch/x86/boot/bzImage");
    let vmlinuz = join_path(staging, "boot/vmlinuz");
    shell("cp " + bzimage + " " + vmlinuz);
    log("  Installed vmlinuz");

    // Install modules
    log("  Installing modules...");
    shell("make -C " + source_path + " O=" + build_dir + " INSTALL_MOD_PATH=" + staging + " modules_install");
    log("  Modules installed");

    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    // Don't remove the submodule or leviso's kernel
    // Only clean downloaded source
    let downloaded = join_path(BUILD_DIR, "linux");
    if is_dir(downloaded) {
        log("Removing downloaded kernel source...");
        rm(downloaded);
    }
    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let build_dir = join_path(acornos_dir, "output/kernel-build");
    let staging = join_path(acornos_dir, "output/staging");

    if exists(build_dir) {
        log("Removing kernel build...");
        rm(build_dir);
    }

    // Remove kernel from staging
    let vmlinuz = join_path(staging, "boot/vmlinuz");
    if is_file(vmlinuz) {
        rm(vmlinuz);
    }

    ctx
}
