// 7z build dependency - provides p7zip for ISO extraction
//
// Used by alpine.rhai to extract Alpine Extended ISO
// Downloads p7zip from Alpine repository and extracts to TOOLS_PREFIX

let build_deps = [];

let ctx = #{
    name: "7z-deps",
    description: "7z (p7zip) for ISO extraction",
    p7zip_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/p7zip-17.05-r0.apk",
    p7zip_file: "p7zip-17.05-r0.apk",
    p7zip_sha256: "e4e71c9eb6d6e58eaf6f2e19bdc3e49e62e46aa8b25e81a8d52aeef77f4bee6f",
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
    let dest = join_path(BUILD_DIR, ctx.p7zip_file);
    if !is_file(dest) {
        log("Downloading p7zip from Alpine repository...");
        let path = download(ctx.p7zip_url, dest);
        if path == "" {
            throw "Failed to download p7zip";
        }
        verify_sha256(path, ctx.p7zip_sha256);
    }
    ctx
}

fn install(ctx) {
    mkdir(TOOLS_PREFIX);
    let apk_file = join_path(BUILD_DIR, ctx.p7zip_file);
    log("Extracting p7zip to TOOLS_PREFIX...");
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
