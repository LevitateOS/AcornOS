// recfstab - fstab generator recipe for AcornOS
//
// Provides: recfstab binary for generating /etc/fstab
// Output: Binary in output/staging/usr/bin/recfstab
//
// recfstab is distro-agnostic - same binary works for LevitateOS and AcornOS.
//
// Phases:
// - acquire: Get source from submodule
// - build: Compile with cargo
// - install: Copy binary to staging

let ctx = #{
    description: "recfstab - fstab generator (like genfstab)",
    name: "recfstab",
    binary_name: "recfstab",
    // State
    source_path: "",
    binary_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recfstab");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        return ctx;
    }

    throw "recfstab source not found";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recfstab");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        log("Found recfstab source at " + source);
        return ctx;
    }

    throw "recfstab source not found in monorepo at tools/recfstab";
}

// === BUILD ===

fn is_built(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let binary = join_path(monorepo, "target/release/recfstab");

    if is_file(binary) {
        ctx.binary_path = binary;
        return ctx;
    }

    throw "recfstab not built";
}

fn build(ctx) {
    log("Building recfstab...");

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);

    // Build in release mode
    shell("cargo build --release -p recfstab --manifest-path " + join_path(monorepo, "Cargo.toml"));

    let binary = join_path(monorepo, "target/release/recfstab");
    if !is_file(binary) {
        throw "recfstab build failed - binary not found";
    }

    ctx.binary_path = binary;
    log("recfstab built successfully");
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/recfstab");

    if is_file(installed) {
        return ctx;
    }

    throw "recfstab not installed";
}

fn install(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let dest = join_path(staging, "usr/bin/recfstab");

    mkdir(join_path(staging, "usr/bin"));
    shell("cp " + ctx.binary_path + " " + dest);
    chmod(dest, 0o755);

    log("recfstab installed to staging");
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/recfstab");

    if is_file(installed) {
        rm(installed);
        log("recfstab removed from staging");
    }
    ctx
}
