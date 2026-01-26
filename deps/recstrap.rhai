// recstrap - installation tool recipe for AcornOS
//
// Provides: recstrap binary for AcornOS installation
// Output: Binary in output/staging/usr/bin/recstrap
//
// recstrap is distro-agnostic - same binary works for LevitateOS and AcornOS.
//
// Phases:
// - acquire: Get source from submodule
// - build: Compile with cargo
// - install: Copy binary to staging

let ctx = #{
    description: "recstrap - system extractor (like pacstrap)",
    name: "recstrap",
    binary_name: "recstrap",
    // State
    source_path: "",
    binary_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recstrap");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        return ctx;
    }

    throw "recstrap source not found";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let source = join_path(monorepo, "tools/recstrap");

    if is_file(join_path(source, "Cargo.toml")) {
        ctx.source_path = source;
        log("Found recstrap source at " + source);
        return ctx;
    }

    throw "recstrap source not found in monorepo at tools/recstrap";
}

// === BUILD ===

fn is_built(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);
    let binary = join_path(monorepo, "target/release/recstrap");

    if is_file(binary) {
        ctx.binary_path = binary;
        return ctx;
    }

    throw "recstrap not built";
}

fn build(ctx) {
    log("Building recstrap...");

    let acornos_dir = dirname(BUILD_DIR);
    let monorepo = dirname(acornos_dir);

    // Build in release mode
    shell("cargo build --release -p recstrap --manifest-path " + join_path(monorepo, "Cargo.toml"));

    let binary = join_path(monorepo, "target/release/recstrap");
    if !is_file(binary) {
        throw "recstrap build failed - binary not found";
    }

    ctx.binary_path = binary;
    log("recstrap built successfully");
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/recstrap");

    if is_file(installed) {
        return ctx;
    }

    throw "recstrap not installed";
}

fn install(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let dest = join_path(staging, "usr/bin/recstrap");

    mkdir(join_path(staging, "usr/bin"));
    shell("cp " + ctx.binary_path + " " + dest);
    chmod(dest, 0o755);

    log("recstrap installed to staging");
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    // Nothing to clean - binary is in shared target/
    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let acornos_dir = dirname(BUILD_DIR);
    let staging = join_path(acornos_dir, "output/staging");
    let installed = join_path(staging, "usr/bin/recstrap");

    if is_file(installed) {
        rm(installed);
        log("recstrap removed from staging");
    }
    ctx
}
