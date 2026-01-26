// recchroot - chroot helper recipe for AcornOS
//
// Provides: recchroot binary for chroot operations
// Output: Binary in output/staging/usr/bin/recchroot
//
// recchroot is distro-agnostic - same binary works for LevitateOS and AcornOS.
//
// Phases:
// - acquire: Get source from submodule
// - build: Compile with cargo
// - install: Copy binary to staging

let ctx = #{
    description: "recchroot - chroot helper (like arch-chroot)",
    name: "recchroot",
    binary_name: "recchroot",
    // State
    source_path: "",
    binary_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recchroot");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        return ctx;
    }

    throw "recchroot source not found";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recchroot");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        log("Found recchroot source at " + source);
        return ctx;
    }

    throw "recchroot source not found in monorepo at tools/recchroot";
}

// === BUILD ===

fn is_built(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let binary = join_path(monorepo, "target/release/recchroot");

    if is_file(binary) {
        ctx.binary_path = binary;
        return ctx;
    }

    throw "recchroot not built";
}

fn build(ctx) {
    log("Building recchroot...");

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);

    // Build in release mode
    shell("cargo build --release -p recchroot --manifest-path " + join_path(monorepo, "Cargo.toml"));

    let binary = join_path(monorepo, "target/release/recchroot");
    if !is_file(binary) {
        throw "recchroot build failed - binary not found";
    }

    ctx.binary_path = binary;
    log("recchroot built successfully");
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/recchroot");

    if is_file(installed) {
        return ctx;
    }

    throw "recchroot not installed";
}

fn install(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let dest = join_path(staging, "usr/bin/recchroot");

    mkdir(join_path(staging, "usr/bin"));
    shell("cp " + ctx.binary_path + " " + dest);
    chmod(dest, 0o755);

    log("recchroot installed to staging");
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
    let installed = join_path(staging, "usr/bin/recchroot");

    if is_file(installed) {
        rm(installed);
        log("recchroot removed from staging");
    }
    ctx
}
