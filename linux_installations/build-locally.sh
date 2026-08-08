#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPOSITORY_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

INSTALL_MISSING_TOOLS=0
TOTAL_CORES=${LOCORYN_BUILD_CORES:-}

usage() {
    cat <<'EOF'
Build every supported Locoryn Linux and Windows architecture from Linux.

Usage: ./linux_installations/build-locally.sh [options]

Options:
  --cores NUMBER       Total CPU cores shared between the four builds.
                       Defaults to every online core on the system.
  --install-tools      Install missing cross and cargo-xwin commands with Cargo.
  --help, -h           Show this help.

Outputs are written to:
  ./LOCAL-BUILDS/locoryn-VERSION-TIMESTAMP/READY-TO-INSTALL

Required system tools:
  - Rust installed with rustup
  - Docker 20.10+ or Podman 3.4+ running for Linux cross-builds
  - LLVM/Clang (clang-cl, lld-link, and llvm-rc)
  - zip, tar, and sha256sum

The script builds Linux x86_64, Linux ARM64, Windows x86_64, and Windows
ARM64 concurrently. Windows outputs are portable ZIP packages. Linux outputs
include both release archives and self-contained local-installer archives.
EOF
}

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --cores)
            [[ $# -ge 2 ]] || fail '--cores requires a number.'
            TOTAL_CORES=$2
            shift 2
            ;;
        --cores=*)
            TOTAL_CORES=${1#*=}
            shift
            ;;
        --install-tools)
            INSTALL_MISSING_TOOLS=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            fail "Unknown option: $1 (use --help for usage)"
            ;;
    esac
done

[[ $(uname -s) == Linux ]] || fail 'This builder must be run on Linux.'

for command_name in awk cargo cp date install ln ls mkdir rustup sha256sum tail tar uname zip; do
    command -v "$command_name" >/dev/null 2>&1 \
        || fail "Required command not found: $command_name"
done

if [[ -z $TOTAL_CORES ]]; then
    TOTAL_CORES=$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)
    if [[ ! $TOTAL_CORES =~ ^[1-9][0-9]*$ ]] && command -v nproc >/dev/null 2>&1; then
        TOTAL_CORES=$(nproc)
    fi
fi
[[ $TOTAL_CORES =~ ^[1-9][0-9]*$ ]] \
    || fail 'Could not detect a positive CPU count; pass --cores NUMBER.'

if ! command -v cross >/dev/null 2>&1; then
    if (( INSTALL_MISSING_TOOLS )); then
        printf 'Installing cross with Cargo...\n'
        CARGO_BUILD_JOBS=$TOTAL_CORES cargo install --locked \
            --git https://github.com/cross-rs/cross cross
    else
        fail "cross is missing. Install it, or rerun with --install-tools."
    fi
fi

if ! command -v cargo-xwin >/dev/null 2>&1; then
    if (( INSTALL_MISSING_TOOLS )); then
        printf 'Installing cargo-xwin with Cargo...\n'
        CARGO_BUILD_JOBS=$TOTAL_CORES cargo install --locked cargo-xwin
    else
        fail "cargo-xwin is missing. Install it, or rerun with --install-tools."
    fi
fi
hash -r

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
    export CROSS_CONTAINER_ENGINE=docker
elif command -v podman >/dev/null 2>&1 && podman info >/dev/null 2>&1; then
    export CROSS_CONTAINER_ENGINE=podman
else
    fail 'Docker or Podman is required and must be running for the Linux cross-builds.'
fi

resolve_llvm_tool() {
    local tool_name=$1
    local candidate

    if command -v "$tool_name" >/dev/null 2>&1; then
        command -v "$tool_name"
        return 0
    fi

    for version in {30..14}; do
        candidate="$tool_name-$version"
        if command -v "$candidate" >/dev/null 2>&1; then
            command -v "$candidate"
            return 0
        fi
    done

    return 1
}

LLVM_SHIMS="$REPOSITORY_ROOT/target/local-builds/tool-shims"
mkdir -p "$LLVM_SHIMS"
for llvm_tool in clang-cl lld-link llvm-rc; do
    llvm_path=$(resolve_llvm_tool "$llvm_tool") \
        || fail "LLVM tool not found: $llvm_tool (install a complete LLVM/Clang toolchain)."
    ln -sfn "$llvm_path" "$LLVM_SHIMS/$llvm_tool"
done
export PATH="$LLVM_SHIMS:$PATH"
export RC_PATH="$LLVM_SHIMS/llvm-rc"

VERSION=$(awk '
    /^\[package\][[:space:]]*$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
        value = $0
        sub(/^[^=]*=[[:space:]]*"/, "", value)
        sub(/"[[:space:]]*$/, "", value)
        print value
        exit
    }
' "$REPOSITORY_ROOT/Cargo.toml")
[[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][A-Za-z0-9._-]+)?$ ]] \
    || fail "Could not read a valid package version from Cargo.toml: $VERSION"

RUN_TIMESTAMP=$(date -u +%Y%m%d-%H%M%S)
RUN_DIRECTORY="$REPOSITORY_ROOT/LOCAL-BUILDS/locoryn-$VERSION-$RUN_TIMESTAMP-$$"
READY_DIRECTORY="$RUN_DIRECTORY/READY-TO-INSTALL"
PACKAGE_DIRECTORY="$RUN_DIRECTORY/packages"
LOG_DIRECTORY="$RUN_DIRECTORY/logs"
TARGET_DIRECTORY="$REPOSITORY_ROOT/target/local-builds"
mkdir -p "$READY_DIRECTORY" "$PACKAGE_DIRECTORY" "$LOG_DIRECTORY" "$TARGET_DIRECTORY"

printf 'Preparing Rust Windows targets and the Microsoft SDK cache...\n'
rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc
cargo xwin cache xwin

TARGET_LABELS=(linux-x86_64 linux-arm64 windows-x86_64 windows-arm64)
TARGET_TRIPLES=(
    x86_64-unknown-linux-gnu
    aarch64-unknown-linux-gnu
    x86_64-pc-windows-msvc
    aarch64-pc-windows-msvc
)
TARGET_BUILDERS=(cross cross xwin xwin)
TARGET_PIDS=()

base_jobs=$((TOTAL_CORES / 4))
remaining_jobs=$((TOTAL_CORES % 4))
if (( base_jobs < 1 )); then
    base_jobs=1
    remaining_jobs=0
fi

cd "$REPOSITORY_ROOT"
printf '\nBuilding Locoryn %s for four targets with %s detected core(s).\n' \
    "$VERSION" "$TOTAL_CORES"
printf 'Build logs: %s\n\n' "$LOG_DIRECTORY"

for index in "${!TARGET_LABELS[@]}"; do
    label=${TARGET_LABELS[$index]}
    target=${TARGET_TRIPLES[$index]}
    builder=${TARGET_BUILDERS[$index]}
    jobs=$base_jobs
    if (( index < remaining_jobs )); then
        jobs=$((jobs + 1))
    fi
    printf '  Starting %-18s (%s Cargo job(s))\n' "$label" "$jobs"
    if [[ $builder == cross ]]; then
        (
            CARGO_BUILD_JOBS=$jobs cross build --locked --release \
                --jobs "$jobs" \
                --target "$target" \
                --target-dir "$TARGET_DIRECTORY/$label"
        ) >"$LOG_DIRECTORY/$label.log" 2>&1 &
    else
        (
            CARGO_BUILD_JOBS=$jobs cargo xwin build --locked --release \
                --jobs "$jobs" \
                --target "$target" \
                --target-dir "$TARGET_DIRECTORY/$label"
        ) >"$LOG_DIRECTORY/$label.log" 2>&1 &
    fi
    TARGET_PIDS+=("$!")
done

build_failed=0
for index in "${!TARGET_PIDS[@]}"; do
    label=${TARGET_LABELS[$index]}
    if wait "${TARGET_PIDS[$index]}"; then
        printf '  Finished %-18s\n' "$label"
    else
        printf '  FAILED   %s (last 30 log lines follow)\n' "$label" >&2
        tail -n 30 "$LOG_DIRECTORY/$label.log" >&2 || true
        build_failed=1
    fi
done
(( build_failed == 0 )) || fail "One or more builds failed. Full logs are in $LOG_DIRECTORY"

stage_portable_package() {
    local label=$1
    local target=$2
    local os_name=$3
    local architecture=$4
    local executable_name=$5
    local package_name="locoryn-$VERSION-$os_name-$architecture"
    local package_path="$PACKAGE_DIRECTORY/$package_name"
    local built_binary="$TARGET_DIRECTORY/$label/$target/release/$executable_name"

    [[ -f $built_binary ]] || fail "Built executable is missing: $built_binary"
    mkdir -p "$package_path"
    install -m 755 "$built_binary" "$package_path/$executable_name"
    cp -R "$REPOSITORY_ROOT/assets" "$REPOSITORY_ROOT/config" "$package_path/"

    if [[ $os_name == linux ]]; then
        tar -czf "$READY_DIRECTORY/$package_name.tar.gz" \
            -C "$PACKAGE_DIRECTORY" "$package_name"
    else
        (
            cd "$PACKAGE_DIRECTORY"
            zip -qr "$READY_DIRECTORY/$package_name.zip" "$package_name"
        )
    fi
}

stage_linux_installer() {
    local architecture=$1
    local package_name="locoryn-$VERSION-linux-$architecture"
    local installer_name="$package_name-local-installer"
    local installer_path="$PACKAGE_DIRECTORY/$installer_name"

    mkdir -p "$installer_path/payload"
    cp -R "$PACKAGE_DIRECTORY/$package_name/." "$installer_path/payload/"
    printf '%s\n' "$VERSION" > "$installer_path/payload/VERSION"
    install -m 755 "$REPOSITORY_ROOT/linux_installations/install-local.sh" \
        "$installer_path/install-linux.sh"
    install -m 755 "$REPOSITORY_ROOT/linux_installations/uninstall-linux.sh" \
        "$installer_path/uninstall-linux.sh"
    install -m 644 \
        "$REPOSITORY_ROOT/linux_installations/io.github.logancammish.locoryn.desktop.in" \
        "$installer_path/io.github.logancammish.locoryn.desktop.in"
    install -m 644 "$REPOSITORY_ROOT/linux_installations/LOCAL_INSTALL_README.md" \
        "$installer_path/README.md"
    tar -czf "$READY_DIRECTORY/$installer_name.tar.gz" \
        -C "$PACKAGE_DIRECTORY" "$installer_name"
}

printf '\nPackaging builds...\n'
stage_portable_package linux-x86_64 x86_64-unknown-linux-gnu linux x86_64 locoryn
stage_portable_package linux-arm64 aarch64-unknown-linux-gnu linux arm64 locoryn
stage_portable_package windows-x86_64 x86_64-pc-windows-msvc windows x86_64 locoryn.exe
stage_portable_package windows-arm64 aarch64-pc-windows-msvc windows arm64 locoryn.exe
stage_linux_installer x86_64
stage_linux_installer arm64

(
    cd "$READY_DIRECTORY"
    sha256sum ./*.tar.gz ./*.zip > "SHA256SUMS-$VERSION.txt"
)

cat > "$READY_DIRECTORY/README.txt" <<EOF
Locoryn $VERSION local builds

Windows:
  Extract the ZIP for the required architecture and run locoryn.exe.

Linux:
  Extract the matching *-local-installer.tar.gz file, enter its directory,
  and run: sh install-linux.sh

The shorter Linux tar.gz files are portable/release packages. SHA-256 hashes
for every archive are in SHA256SUMS-$VERSION.txt.
EOF

printf '\n============================================================\n'
printf 'ALL FOUR LOCORYN BUILDS COMPLETED SUCCESSFULLY\n'
printf 'READY-TO-INSTALL FILES:\n  %s\n' "$READY_DIRECTORY"
printf '============================================================\n\n'
ls -lh "$READY_DIRECTORY"
