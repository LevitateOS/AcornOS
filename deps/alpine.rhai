// Alpine Linux Extended ISO - dependency recipe
//
// Provides: extracted Alpine rootfs for acornos to build upon
// Output: PREFIX/rootfs
//
// === Globals (provided by executor) ===
//
// BUILD_DIR  - Temporary build directory for intermediate files
// PREFIX     - Installation prefix (where final artifacts go)
//
// === Helper functions (provided by executor) ===
//
// Filesystem:
//   exists(path)              - Check if path exists
//   is_file(path)             - Check if path is a file
//   is_dir(path)              - Check if path is a directory
//   mkdir(path)               - Create directory (recursive)
//   rm(path)                  - Remove file or directory
//   mv(src, dst)              - Move/rename file or directory
//   cp(src, dst)              - Copy file or directory
//   ln(target, link)          - Create symlink
//   chmod(path, mode)         - Change permissions (mode as octal int, e.g. 0o755)
//   file_size(path)           - Get file size in bytes
//   list_dir(path)            - List directory contents as array of strings
//
// Paths:
//   join_path(a, b)           - Join path components safely
//   basename(path)            - Get filename from path
//   dirname(path)             - Get directory from path
//   extension(path)           - Get file extension (without dot)
//
// Commands:
//   shell(cmd)                - Run shell command, throw on failure
//   shell_output(cmd)         - Run command, return stdout as string
//   shell_status(cmd)         - Run command, return exit code (don't throw)
//
// Network:
//   download(url, dest)       - HTTP download, returns path or ""
//   http_get(url)             - Fetch URL content as string
//
// Verification:
//   verify_sha256(path, hash) - Verify file checksum, throw on mismatch
//   fetch_sha256(url, name)   - Fetch CHECKSUM file, extract hash for filename
//   check_disk_space(path, n) - Verify n bytes free at path, throw if not
//
// Archive:
//   extract(archive, dest)    - Auto-detect format and extract (tar, zip, 7z, squashfs)
//
// Parsing:
//   parse_int(str)            - Parse string to integer
//
// Version:
//   version_compare(a, b)     - Compare semver: -1 (a<b), 0 (a==b), 1 (a>b)
//   version_newer(a, b)       - Returns true if a > b
//
// Environment:
//   env(name)                 - Get environment variable, "" if not set
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
//   warn(msg)                 - Print warning message

let ctx = #{
    // === Config ===
    name: "alpine",
    version: "3.23.2",
    description: "Alpine Linux rootfs (extracted from Extended ISO)",

    iso_name: "alpine-extended-3.23.2-x86_64.iso",
    iso_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-extended-3.23.2-x86_64.iso",
    sha256_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-extended-3.23.2-x86_64.iso.sha256",
    sha256: "",
    size_bytes: 1413529600,
    disk_buffer: 536870912,  // 512MB buffer

    apk_tools_name: "apk-tools-static-3.0.3-r1.apk",
    apk_tools_url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/apk-tools-static-3.0.3-r1.apk",
    apk_tools_sha256: "b7bb8715648d32854f06392468f69cd1dd4904d5fac9f9e43fc5d6942d53597c",

    // === State ===
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

    // Version check: filename must match expected
    let actual_name = basename(ctx.iso_path);
    if actual_name != ctx.iso_name {
        throw "version mismatch: have " + actual_name + ", want " + ctx.iso_name;
    }

    // Integrity check (skip if sha256 not yet fetched)
    if ctx.sha256 != "" {
        verify_sha256(ctx.iso_path, ctx.sha256);
    }

    // Also need apk-tools
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

    // --- ISO ---
    log("Fetching checksum for " + ctx.iso_name);
    ctx.sha256 = fetch_alpine_checksum(ctx.sha256_url);

    // Check if ISO already exists and is valid
    if is_file(iso_dest) {
        log("Found existing ISO, verifying checksum...");
        let status = shell_status("sha256sum -c --status <<< '" + ctx.sha256 + "  " + iso_dest + "'");
        if status == 0 {
            log("Existing ISO is valid, skipping download");
            ctx.iso_path = iso_dest;
        } else {
            log("Existing ISO is corrupted or wrong version, re-downloading...");
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

        log("Verifying checksum...");
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
            throw "apk.static not found after extraction at: " + apk_static;
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

    // Check artifacts exist
    if !is_dir(join_path(rootfs_temp, "bin")) {
        throw "rootfs not built";
    }

    // Version check
    let built_version = trim(read_file(version_file));
    if built_version == "" {
        throw "version marker missing - rebuild required";
    }
    if built_version != ctx.version {
        throw "version mismatch: built " + built_version + ", want " + ctx.version;
    }

    ctx
}

fn build(ctx) {
    let iso_contents = join_path(BUILD_DIR, "iso-contents");
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let version_file = join_path(BUILD_DIR, ".alpine-built-version");

    // Check for stale artifacts from a different version
    let existing_version = trim(read_file(version_file));
    if existing_version != "" && existing_version != ctx.version {
        log("Cleaning stale build artifacts from version " + existing_version);
        if is_dir(iso_contents) { rm(iso_contents); }
        if is_dir(rootfs_temp) { rm(rootfs_temp); }
        rm(version_file);
    }

    // Extract ISO
    if !is_dir(join_path(iso_contents, "apks")) {
        log("Extracting ISO to " + iso_contents);
        mkdir(iso_contents);
        extract(ctx.iso_path, iso_contents);
    }

    // Verify apks directory exists
    let apks_dir = join_path(iso_contents, "apks/x86_64");
    if !is_dir(apks_dir) {
        throw "apks/x86_64 not found in ISO at: " + apks_dir;
    }

    // Create rootfs using apk-tools-static
    if !is_dir(join_path(rootfs_temp, "bin")) {
        log("Creating rootfs with apk-tools-static...");
        mkdir(rootfs_temp);

        // Create required directories for apk
        mkdir(join_path(rootfs_temp, "etc/apk"));
        mkdir(join_path(rootfs_temp, "etc/apk/keys"));
        mkdir(join_path(rootfs_temp, "var/cache/apk"));

        // Set up repository to use local ISO packages + remote repos
        let repo_content = join_path(iso_contents, "apks") + "\n" +
            "https://dl-cdn.alpinelinux.org/alpine/v3.23/main\n" +
            "https://dl-cdn.alpinelinux.org/alpine/v3.23/community\n";
        write_file(join_path(rootfs_temp, "etc/apk/repositories"), repo_content);

        // Install base packages
        // NOTE: Full package list is managed by acornos (src/extract.rs), not the recipe
        // This installs minimal packages to create a bootable rootfs
        let packages = "alpine-base busybox openrc";
        shell(ctx.apk_static_path + " --root " + rootfs_temp + " --initdb --no-progress add " + packages);
    }

    // Write version marker
    write_file(version_file, ctx.version);
    log("Build complete for Alpine " + ctx.version);

    ctx
}

// === INSTALL ===

fn is_installed(ctx) {
    let version_file = join_path(PREFIX, ".alpine-version");

    if ctx.rootfs_path == "" {
        throw "not installed";
    }

    if !is_dir(join_path(ctx.rootfs_path, "bin")) {
        throw "rootfs missing at " + ctx.rootfs_path;
    }

    // Version check
    let installed_version = trim(read_file(version_file));
    if installed_version == "" {
        throw "version marker missing - reinstall required";
    }
    if installed_version != ctx.version {
        throw "version mismatch: installed " + installed_version + ", want " + ctx.version;
    }

    ctx
}

fn install(ctx) {
    let rootfs_temp = join_path(BUILD_DIR, "rootfs");
    let rootfs_dest = join_path(PREFIX, "rootfs");
    let version_file = join_path(PREFIX, ".alpine-version");

    // Check for existing installation
    let existing_version = trim(read_file(version_file));
    if existing_version != "" {
        if existing_version == ctx.version {
            log("Alpine " + ctx.version + " already installed, skipping");
            ctx.rootfs_path = rootfs_dest;
            return ctx;
        }
        log("Removing existing installation (version " + existing_version + ")");
        if is_dir(rootfs_dest) { rm(rootfs_dest); }
        rm(version_file);
    }

    log("Installing Alpine " + ctx.version + " to " + PREFIX);

    mkdir(PREFIX);
    mv(rootfs_temp, rootfs_dest);

    // Write version marker
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

    log("Cleaning up build artifacts...");

    if is_dir(iso_contents) {
        rm(iso_contents);
    }
    if is_dir(rootfs_temp) {
        rm(rootfs_temp);
    }
    if is_dir(apk_tools_dir) {
        rm(apk_tools_dir);
    }
    if ctx.iso_path != "" && is_file(ctx.iso_path) {
        rm(ctx.iso_path);
        ctx.iso_path = "";
    }
    if is_file(version_file) {
        rm(version_file);
    }

    ctx.apk_static_path = "";

    log("Cleanup complete");
    ctx
}

// === UPDATE ===

fn update(ctx) {
    let base_url = "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64";

    log("Checking for Alpine Linux updates...");

    // Fetch the releases page to find latest version
    // Alpine releases page lists ISOs with versions in filenames
    let page_content = http_get(base_url + "/");

    let iso_info = parse_alpine_releases(page_content);

    // Check if update is available
    if version_newer(iso_info.version, ctx.version) {
        log("Update available: " + ctx.version + " -> " + iso_info.version);
    } else if iso_info.version == ctx.version {
        log("Already at latest version: " + ctx.version);
    } else {
        warn("Remote version " + iso_info.version + " is older than current " + ctx.version);
    }

    // Update ctx with new version info
    ctx.version = iso_info.version;
    ctx.iso_name = iso_info.filename;
    ctx.iso_url = base_url + "/" + iso_info.filename;
    ctx.sha256_url = base_url + "/" + iso_info.filename + ".sha256";

    ctx
}

// === REMOVE ===

fn remove(ctx) {
    let version_file = join_path(PREFIX, ".alpine-version");
    let rootfs_dest = join_path(PREFIX, "rootfs");

    // Check if anything is installed
    let installed_version = trim(read_file(version_file));
    if installed_version == "" && !is_dir(rootfs_dest) {
        log("Nothing to remove - Alpine not installed");
        return ctx;
    }

    log("Removing Alpine " + (if installed_version != "" { installed_version } else { "(unknown version)" }) + "...");

    if is_dir(rootfs_dest) {
        rm(rootfs_dest);
    }
    if is_file(version_file) {
        rm(version_file);
    }

    ctx.iso_path = "";
    ctx.apk_static_path = "";
    ctx.rootfs_path = "";

    log("Remove complete");
    ctx
}

// === Helpers ===

private fn fetch_alpine_checksum(url) {
    // Alpine checksum format: "sha256  filename"
    let content = http_get(url);
    let sha256 = trim(content.split(" ")[0]);

    if sha256 == "" {
        throw "Could not parse SHA256 from checksum file";
    }
    if sha256.len() != 64 {
        throw "Invalid SHA256 hash length: expected 64, got " + sha256.len();
    }

    sha256
}

private fn parse_alpine_releases(content) {
    // Parse Alpine releases page HTML to find latest extended ISO
    // Looking for: alpine-extended-X.Y.Z-x86_64.iso
    //
    // NOTE: This is fragile. For production, we'd use LLM helpers or
    // parse the YAML/JSON releases file if Alpine provides one.

    let lines = content.split("\n");
    let filename = "";
    let version = "";

    for line in lines {
        if line.contains("alpine-extended-") && line.contains("-x86_64.iso\"") {
            // Extract filename from href
            let start = line.index_of("alpine-extended-");
            if start >= 0 {
                let end = line.index_of(".iso\"", start);
                if end > start {
                    filename = line.sub_string(start, end - start + 4);
                    // Extract version: alpine-extended-3.21.3-x86_64.iso -> 3.21.3
                    version = filename.replace("alpine-extended-", "").replace("-x86_64.iso", "");
                    break;
                }
            }
        }
    }

    if filename == "" {
        throw "Could not find alpine-extended ISO in releases page";
    }
    if version == "" {
        throw "Could not extract version from filename: " + filename;
    }

    #{
        filename: filename,
        version: version
    }
}
