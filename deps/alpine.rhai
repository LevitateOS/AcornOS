// Alpine Linux Extended ISO - dependency recipe
//
// Provides: extracted Alpine rootfs for acornos to build upon
// Output: RECIPE_DIR/output/rootfs
//
// === Globals (provided by executor) ===
//
// BUILD_DIR   - Temporary build directory for intermediate files
// RECIPE_DIR  - Directory containing this recipe file
//
// === Helper functions (provided by executor) ===
//
// Filesystem:
//   is_file(path)             - Check if path is a file
//   is_dir(path)              - Check if path is a directory
//   mkdir(path)               - Create directory (recursive)
//   rm(path)                  - Remove file or directory
//   mv(src, dst)              - Move/rename file or directory
//   chmod(path, mode)         - Change permissions (mode as octal int, e.g. 0o755)
//
// Paths:
//   join_path(a, b)           - Join path components safely
//   basename(path)            - Get filename from path
//
// Commands:
//   shell(cmd)                - Run shell command, throw on failure
//   shell_status(cmd)         - Run command, return exit code (don't throw)
//
// Network:
//   download(url, dest)       - HTTP download, returns path or ""
//   http_get(url)             - Fetch URL content as string
//
// Verification:
//   verify_sha256(path, hash) - Verify file checksum, throw on mismatch
//   check_disk_space(path, n) - Verify n bytes free at path, throw if not
//
// Archive:
//   extract(archive, dest)    - Auto-detect format and extract (tar, zip, 7z, squashfs)
//
// File I/O:
//   read_file(path)           - Read file content as string, "" if not exists
//   write_file(path, content) - Write string to file
//
// String:
//   trim(str)                 - Remove leading/trailing whitespace
//
// Logging:
//   log(msg)                  - Print info message

let ctx = #{
    name: "alpine",
    version: "3.23.2",
    description: "Alpine Linux rootfs (extracted from Extended ISO)",

    iso_name: "alpine-extended-3.23.2-x86_64.iso",
    iso_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-extended-3.23.2-x86_64.iso",
    sha256_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-extended-3.23.2-x86_64.iso.sha256",
    sha256: "",
    size_bytes: 1413529600,
    disk_buffer: 536870912,

    apk_tools_name: "apk-tools-static-3.0.3-r1.apk",
    apk_tools_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/apk-tools-static-3.0.3-r1.apk",
    apk_tools_sha256: "b7bb8715648d32854f06392468f69cd1dd4904d5fac9f9e43fc5d6942d53597c",

    iso_path: "",
    apk_static_path: "",
    rootfs_path: "",
};

// === ACQUIRE ===

fn is_acquired(ctx) {
    if ctx.iso_path == "" {
        throw "ISO not acquired";
    }
    if !is_file(ctx.iso_path) {
        throw "ISO missing at " + ctx.iso_path;
    }
    if ctx.apk_static_path == "" {
        throw "apk-tools-static not acquired";
    }
    if !is_file(ctx.apk_static_path) {
        throw "apk.static missing at " + ctx.apk_static_path;
    }
    ctx
}

fn acquire(ctx) {
    let iso_dest = join_path(BUILD_DIR, ctx.iso_name);
    let apk_tools_dir = join_path(BUILD_DIR, "apk-tools");
    let apk_tools_dest = join_path(apk_tools_dir, ctx.apk_tools_name);
    let apk_static = join_path(apk_tools_dir, "sbin/apk.static");

    mkdir(BUILD_DIR);

    // --- Fetch checksum ---
    log("Fetching checksum for " + ctx.iso_name);
    let checksum_content = http_get(ctx.sha256_url);
    ctx.sha256 = trim(checksum_content.split(" ")[0]);

    // --- ISO ---
    if is_file(iso_dest) {
        log("Found existing ISO, verifying...");
        let status = shell_status("sha256sum -c --status <<< '" + ctx.sha256 + "  " + iso_dest + "'");
        if status == 0 {
            log("Existing ISO valid, skipping download");
            ctx.iso_path = iso_dest;
        } else {
            log("Existing ISO invalid, re-downloading...");
            rm(iso_dest);
        }
    }

    if ctx.iso_path == "" {
        check_disk_space(BUILD_DIR, ctx.size_bytes + ctx.disk_buffer);
        log("Downloading " + ctx.iso_name + "...");
        let path = download(ctx.iso_url, iso_dest);
        if path == "" {
            throw "download failed for " + ctx.iso_name;
        }
        verify_sha256(path, ctx.sha256);
        ctx.iso_path = path;
    }

    // --- apk-tools-static ---
    if is_file(apk_static) {
        log("apk-tools-static already available");
        ctx.apk_static_path = apk_static;
    } else {
        mkdir(apk_tools_dir);
        if !is_file(apk_tools_dest) {
            log("Downloading apk-tools-static...");
            let path = download(ctx.apk_tools_url, apk_tools_dest);
            if path == "" {
                throw "download failed for apk-tools-static";
            }
            verify_sha256(path, ctx.apk_tools_sha256);
        }
        log("Extracting apk-tools-static...");
        extract(apk_tools_dest, apk_tools_dir);
        if !is_file(apk_static) {
            throw "apk.static not found after extraction";
        }
        chmod(apk_static, 0o755);
        ctx.apk_static_path = apk_static;
    }

    ctx
}

// === BUILD ===

fn is_built(ctx) {
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let version_file = join_path(BUILD_DIR, ".alpine-built-version");

    if !is_dir(join_path(rootfs_temp, "bin")) {
        throw "rootfs not built";
    }
    let built_version = trim(read_file(version_file));
    if built_version != ctx.version {
        throw "version mismatch: built " + built_version + ", want " + ctx.version;
    }
    ctx
}

fn build(ctx) {
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let version_file = join_path(BUILD_DIR, ".alpine-built-version");

    // Clean stale artifacts
    let existing_version = trim(read_file(version_file));
    if existing_version != "" && existing_version != ctx.version {
        log("Cleaning stale artifacts from " + existing_version);
        if is_dir(iso_contents) { rm(iso_contents); }
        if is_dir(rootfs_temp) { rm(rootfs_temp); }
        rm(version_file);
    }

    // Extract ISO
    if !is_dir(join_path(iso_contents, "apks")) {
        log("Extracting ISO...");
        mkdir(iso_contents);
        extract(ctx.iso_path, iso_contents);
    }

    // Create rootfs
    if !is_dir(join_path(rootfs_temp, "bin")) {
        log("Creating rootfs with apk...");
        mkdir(rootfs_temp);
        mkdir(join_path(rootfs_temp, "etc/apk"));
        mkdir(join_path(rootfs_temp, "etc/apk/keys"));
        mkdir(join_path(rootfs_temp, "var/cache/apk"));

        let repo_content = join_path(iso_contents, "apks") + "\n" +
            "https://dl-cdn.alpinelinux.org/alpine/v3.23/main\n" +
            "https://dl-cdn.alpinelinux.org/alpine/v3.23/community\n";
        write_file(join_path(rootfs_temp, "etc/apk/repositories"), repo_content);

        shell(ctx.apk_static_path + " --root " + rootfs_temp + " --initdb --no-progress add alpine-base busybox openrc");
    }

    write_file(version_file, ctx.version);
    log("Build complete for Alpine " + ctx.version);
    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let output_dir = join_path(RECIPE_DIR, "output");
    let version_file = join_path(output_dir, ".alpine-version");
    if ctx.rootfs_path == "" {
        throw "not installed";
    }
    if !is_dir(join_path(ctx.rootfs_path, "bin")) {
        throw "rootfs missing";
    }
    let installed_version = trim(read_file(version_file));
    if installed_version != ctx.version {
        throw "version mismatch: installed " + installed_version + ", want " + ctx.version;
    }
    ctx
}

fn install(ctx) {
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let output_dir = join_path(RECIPE_DIR, "output");
    let rootfs_dest = join_path(output_dir, "rootfs");
    let version_file = join_path(output_dir, ".alpine-version");

    let existing_version = trim(read_file(version_file));
    if existing_version == ctx.version {
        log("Alpine " + ctx.version + " already installed");
        ctx.rootfs_path = rootfs_dest;
        return ctx;
    }
    if existing_version != "" {
        log("Removing old installation (" + existing_version + ")");
        if is_dir(rootfs_dest) { rm(rootfs_dest); }
        rm(version_file);
    }

    log("Installing Alpine " + ctx.version);
    mkdir(output_dir);
    mv(rootfs_temp, rootfs_dest);
    write_file(version_file, ctx.version);
    ctx.rootfs_path = rootfs_dest;
    log("Install complete");
    ctx
}

// === CLEANUP ===

fn cleanup(ctx) {
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let apk_tools_dir = join_path(BUILD_DIR, "apk-tools");
    let version_file = join_path(BUILD_DIR, ".alpine-built-version");

    log("Cleaning up...");
    if is_dir(iso_contents) { rm(iso_contents); }
    if is_dir(rootfs_temp) { rm(rootfs_temp); }
    if is_dir(apk_tools_dir) { rm(apk_tools_dir); }
    if ctx.iso_path != "" && is_file(ctx.iso_path) {
        rm(ctx.iso_path);
        ctx.iso_path = "";
    }
    if is_file(version_file) { rm(version_file); }
    ctx.apk_static_path = "";
    log("Cleanup complete");
    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let output_dir = join_path(RECIPE_DIR, "output");
    let version_file = join_path(output_dir, ".alpine-version");
    let rootfs_dest = join_path(output_dir, "rootfs");

    let installed_version = trim(read_file(version_file));
    if installed_version == "" && !is_dir(rootfs_dest) {
        log("Nothing to remove");
        return ctx;
    }

    log("Removing Alpine...");
    if is_dir(rootfs_dest) { rm(rootfs_dest); }
    if is_file(version_file) { rm(version_file); }
    ctx.iso_path = "";
    ctx.apk_static_path = "";
    ctx.rootfs_path = "";
    log("Remove complete");
    ctx
}
