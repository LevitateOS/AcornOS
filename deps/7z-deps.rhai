// 7z build dependency - provides p7zip for ISO extraction
//
// Used by alpine.rhai to extract Alpine Extended ISO
// Downloads p7zip from Alpine repository and extracts to TOOLS_PREFIX

let build_deps = [];

let ctx = #{
    name: "7z-deps",
    description: "7z file archiver for ISO extraction",
    archive_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/7zip-25.01-r0.apk",
    archive_file: "7zip-25.01-r0.apk",
    archive_sha256: "6602ccb86033f4132f7c20e7a551908e14631d6d40b00fb3e0e00ae8914ab405",
};

fn is_installed(ctx) {
    if is_file(join_path(TOOLS_PREFIX, "usr/bin/7z")) {
        return ctx;
    }
    if shell_status("which 7z") == 0 {
        return ctx;
    }
    throw "7z not available in TOOLS_PREFIX or system PATH";
}

fn acquire(ctx) {
    mkdir(BUILD_DIR);
    let dest = join_path(BUILD_DIR, ctx.archive_file);
    if !is_file(dest) {
        log("Downloading 7zip from Alpine repository...");
        let path = download(ctx.archive_url, dest);
        if path == "" {
            throw "Failed to download 7zip";
        }
        verify_sha256(path, ctx.archive_sha256);
    }
    ctx
}

fn install(ctx) {
    mkdir(TOOLS_PREFIX);
    let apk_file = join_path(BUILD_DIR, ctx.archive_file);
    log("Extracting 7zip to TOOLS_PREFIX...");
    // APK files are tar.gz, extract them
    shell("cd " + TOOLS_PREFIX + " && tar xzf '" + apk_file + "'");
    // Check if 7z was extracted
    if !is_file(join_path(TOOLS_PREFIX, "usr/bin/7z")) {
        throw "7z not found after extraction";
    }
    log("7z installed to TOOLS_PREFIX");
    ctx
}

ctx
