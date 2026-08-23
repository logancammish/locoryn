#!/bin/sh
set -eu

APP_ID=io.github.logancammish.locoryn
REPOSITORY=logancammish/locoryn
GITHUB_API=https://api.github.com/repos/$REPOSITORY
GITHUB_RELEASES=https://github.com/$REPOSITORY/releases

SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
    /*) ;;
    *) SCRIPT_PATH=$PWD/$SCRIPT_PATH ;;
esac
BUNDLE_DIR=$(CDPATH= cd "$(dirname "$SCRIPT_PATH")" && pwd)
DESKTOP_TEMPLATE="$BUNDLE_DIR/$APP_ID.desktop.in"

ARCH=
CHANNEL=
RELEASE_TAG=
ASSUME_YES=0
DOWNLOADER=
TEMP_DIR=

say() {
    printf '%s\n' "$*"
}

warn() {
    printf 'Warning: %s\n' "$*" >&2
}

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Install Locoryn for the current Linux user.

Usage: ./install-linux.sh [options]

Options:
  --arch ARCH       auto, x86_64 (x64/amd64), or arm64 (aarch64)
  --channel NAME    main/stable or beta
  --tag TAG         install an exact GitHub release tag
  --yes, -y         accept detected/default choices without prompting
  --help, -h        show this help

Environment overrides:
  LOCORYN_INSTALL_DIR   application directory (default: ~/.local/lib/locoryn)
  LOCORYN_BIN_DIR       command directory (default: ~/.local/bin)
EOF
}

normalize_arch() {
    case "$1" in
        auto|'') printf '%s' auto ;;
        x86_64|x86-64|x64|amd64) printf '%s' x86_64 ;;
        arm64|aarch64|armv8|armv8l) printf '%s' arm64 ;;
        *) return 1 ;;
    esac
}

normalize_channel() {
    case "$1" in
        main|stable) printf '%s' main ;;
        beta|prerelease|pre-release) printf '%s' beta ;;
        *) return 1 ;;
    esac
}

detect_arch() {
    case "$(uname -m 2>/dev/null || true)" in
        x86_64|amd64) printf '%s' x86_64 ;;
        aarch64|arm64) printf '%s' arm64 ;;
        *) printf '%s' unknown ;;
    esac
}

fetch_url() {
    if [ "$DOWNLOADER" = curl ]; then
        curl --fail --silent --show-error --location \
            --retry 3 --connect-timeout 15 \
            --header 'Accept: application/vnd.github+json' \
            --header 'X-GitHub-Api-Version: 2022-11-28' \
            --user-agent 'locoryn-linux-installer' "$1"
    else
        wget --quiet --tries=3 --timeout=30 -O - \
            --header='Accept: application/vnd.github+json' \
            --header='X-GitHub-Api-Version: 2022-11-28' \
            --user-agent='locoryn-linux-installer' "$1"
    fi
}

download_url() {
    if [ "$DOWNLOADER" = curl ]; then
        curl --fail --show-error --location \
            --retry 3 --connect-timeout 15 --output "$2" "$1"
    else
        wget --tries=3 --timeout=30 -O "$2" "$1"
    fi
}

cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --arch)
            [ "$#" -ge 2 ] || fail '--arch requires a value'
            ARCH=$(normalize_arch "$2") || fail "Unsupported architecture: $2"
            shift 2
            ;;
        --arch=*)
            ARCH=$(normalize_arch "${1#*=}") || fail "Unsupported architecture: ${1#*=}"
            shift
            ;;
        --channel)
            [ "$#" -ge 2 ] || fail '--channel requires a value'
            CHANNEL=$(normalize_channel "$2") || fail "Unsupported channel: $2"
            shift 2
            ;;
        --channel=*)
            CHANNEL=$(normalize_channel "${1#*=}") || fail "Unsupported channel: ${1#*=}"
            shift
            ;;
        --tag)
            [ "$#" -ge 2 ] || fail '--tag requires a value'
            RELEASE_TAG=$2
            shift 2
            ;;
        --tag=*)
            RELEASE_TAG=${1#*=}
            shift
            ;;
        --yes|-y)
            ASSUME_YES=1
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

[ "$(uname -s 2>/dev/null || true)" = Linux ] || fail 'This installer only supports Linux.'
[ -n "${HOME:-}" ] || fail 'HOME is not set.'
[ -f "$DESKTOP_TEMPLATE" ] || fail "Missing desktop template: $DESKTOP_TEMPLATE"

for tool in awk cp dirname find install ln mkdir mktemp readlink sed tar uname; do
    command -v "$tool" >/dev/null 2>&1 || fail "Required command not found: $tool"
done

if command -v curl >/dev/null 2>&1; then
    DOWNLOADER=curl
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER=wget
else
    fail 'Install curl or wget, then run the installer again.'
fi

DETECTED_ARCH=$(detect_arch)
if [ "$ARCH" = auto ]; then
    ARCH=
fi

if [ -z "$ARCH" ]; then
    if [ "$ASSUME_YES" -eq 1 ]; then
        [ "$DETECTED_ARCH" != unknown ] || fail 'Could not detect the CPU. Pass --arch x86_64 or --arch arm64.'
        ARCH=$DETECTED_ARCH
    else
        say "Detected architecture: $DETECTED_ARCH"
        printf 'Architecture [x86_64/arm64] [%s]: ' "$DETECTED_ARCH"
        IFS= read -r ARCH_INPUT || ARCH_INPUT=
        if [ -z "$ARCH_INPUT" ]; then
            [ "$DETECTED_ARCH" != unknown ] || fail 'Enter x86_64 or arm64.'
            ARCH=$DETECTED_ARCH
        else
            ARCH=$(normalize_arch "$ARCH_INPUT") || fail "Unsupported architecture: $ARCH_INPUT"
            [ "$ARCH" != auto ] || ARCH=$DETECTED_ARCH
            [ "$ARCH" != unknown ] || fail 'Enter x86_64 or arm64.'
        fi
    fi
fi

if [ -z "$CHANNEL" ]; then
    if [ "$ASSUME_YES" -eq 1 ]; then
        CHANNEL=main
    else
        printf 'Release channel [main/beta] [main]: '
        IFS= read -r CHANNEL_INPUT || CHANNEL_INPUT=
        CHANNEL_INPUT=${CHANNEL_INPUT:-main}
        CHANNEL=$(normalize_channel "$CHANNEL_INPUT") || fail "Unsupported channel: $CHANNEL_INPUT"
    fi
fi

case "$RELEASE_TAG" in
    *[!A-Za-z0-9._+-]* ) fail 'The release tag contains unsupported characters.' ;;
esac

DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
INSTALL_DIR=${LOCORYN_INSTALL_DIR:-"$HOME/.local/lib/locoryn"}
BIN_DIR=${LOCORYN_BIN_DIR:-"$HOME/.local/bin"}
APPLICATIONS_DIR="$DATA_HOME/applications"
ICON_DIR="$DATA_HOME/icons/hicolor/512x512/apps"
LAUNCHER="$BIN_DIR/locoryn"
DESKTOP_FILE="$APPLICATIONS_DIR/$APP_ID.desktop"
ICON_FILE="$ICON_DIR/$APP_ID.png"

case "$INSTALL_DIR" in
    "$HOME"|"$HOME/"|"$HOME/.local"|"$HOME/.local/"|"$HOME/.local/lib"|"$HOME/.local/lib/"|/|"")
        fail "Unsafe installation path: $INSTALL_DIR"
        ;;
    "$HOME"/*) ;;
    *) fail "The installation directory must be inside your home directory: $INSTALL_DIR" ;;
esac
case "$BIN_DIR" in
    "$HOME"|"$HOME/"|/|"") fail "Unsafe command directory: $BIN_DIR" ;;
    "$HOME"/*) ;;
    *) fail "The command directory must be inside your home directory: $BIN_DIR" ;;
esac

if [ -d "$INSTALL_DIR" ] && [ ! -f "$INSTALL_DIR/.installed-release" ]; then
    EXISTING_FILE=$(find "$INSTALL_DIR" -mindepth 1 -print | sed -n '1p')
    [ -z "$EXISTING_FILE" ] \
        || fail "$INSTALL_DIR is not empty and was not created by this installer."
fi

if [ -z "$RELEASE_TAG" ]; then
    if [ "$CHANNEL" = main ]; then
        if ! RELEASE_JSON=$(fetch_url "$GITHUB_API/releases/latest"); then
            fail 'Could not retrieve the latest stable release from GitHub.'
        fi
        RELEASE_TAG=$(printf '%s\n' "$RELEASE_JSON" | awk -F '"' '/"tag_name"[[:space:]]*:/ { print $4; exit }')
    else
        if ! RELEASE_JSON=$(fetch_url "$GITHUB_API/releases?per_page=30"); then
            fail 'Could not retrieve beta releases from GitHub.'
        fi
        RELEASE_TAG=$(printf '%s\n' "$RELEASE_JSON" | awk -F '"' '
            /"tag_name"[[:space:]]*:/ { tag = $4 }
            /"prerelease"[[:space:]]*:[[:space:]]*true/ { print tag; exit }
        ')
    fi
fi

[ -n "$RELEASE_TAG" ] || {
    if [ "$CHANNEL" = beta ]; then
        fail 'GitHub does not currently list a beta release.'
    fi
    fail 'GitHub returned a release without a tag.'
}
case "$RELEASE_TAG" in
    *[!A-Za-z0-9._+-]* ) fail 'GitHub returned a release tag with unsupported characters.' ;;
esac

VERSION=${RELEASE_TAG#v}
VERSION=${VERSION#V}
[ -n "$VERSION" ] || fail "Invalid release tag: $RELEASE_TAG"
ASSET_NAME="locoryn-$VERSION-linux-$ARCH.tar.gz"
ASSET_URL="$GITHUB_RELEASES/download/$RELEASE_TAG/$ASSET_NAME"
CHECKSUM_NAME="SHA256SUMS-$VERSION.txt"
CHECKSUM_URL="$GITHUB_RELEASES/download/$RELEASE_TAG/$CHECKSUM_NAME"

say ""
say "Locoryn Linux installation"
say "  Release:      $RELEASE_TAG ($CHANNEL)"
say "  Architecture: $ARCH"
say "  Destination:  $INSTALL_DIR"

if [ "$ASSUME_YES" -ne 1 ]; then
    printf 'Continue? [Y/n]: '
    IFS= read -r CONFIRM || CONFIRM=
    case "$CONFIRM" in
        ''|y|Y|yes|YES|Yes) ;;
        *) say 'Installation cancelled.'; exit 0 ;;
    esac
fi

TEMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/locoryn-install.XXXXXX") || fail 'Could not create a temporary directory.'
trap cleanup EXIT HUP INT TERM
ARCHIVE="$TEMP_DIR/$ASSET_NAME"
CHECKSUM_FILE="$TEMP_DIR/$CHECKSUM_NAME"
EXTRACT_DIR="$TEMP_DIR/extracted"

say "Downloading $ASSET_NAME ..."
download_url "$ASSET_URL" "$ARCHIVE" || fail "Could not download release asset: $ASSET_URL"

if download_url "$CHECKSUM_URL" "$CHECKSUM_FILE" >/dev/null 2>&1; then
    EXPECTED_HASH=$(awk -v wanted="$ASSET_NAME" '
        {
            file = $2
            sub(/^\*/, "", file)
            count = split(file, parts, "/")
            if (parts[count] == wanted) { print tolower($1); exit }
        }
    ' "$CHECKSUM_FILE")
    if [ -n "$EXPECTED_HASH" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            ACTUAL_HASH=$(sha256sum "$ARCHIVE" | awk '{ print $1 }')
        elif command -v shasum >/dev/null 2>&1; then
            ACTUAL_HASH=$(shasum -a 256 "$ARCHIVE" | awk '{ print $1 }')
        else
            ACTUAL_HASH=
            warn 'sha256sum/shasum is unavailable; the downloaded checksum could not be verified.'
        fi
        if [ -n "$ACTUAL_HASH" ]; then
            [ "$ACTUAL_HASH" = "$EXPECTED_HASH" ] || fail 'The downloaded archive failed SHA-256 verification.'
            say 'SHA-256 checksum verified.'
        fi
    else
        warn "$CHECKSUM_NAME does not contain an entry for $ASSET_NAME."
    fi
else
    warn 'This release has no checksum file; continuing with GitHub HTTPS verification only.'
fi

if ! tar -tzf "$ARCHIVE" | while IFS= read -r MEMBER; do
    case "$MEMBER" in
        /*|../*|*/../*|*/..) exit 1 ;;
    esac
done; then
    fail 'The release archive contains an unsafe path.'
fi

mkdir -p "$EXTRACT_DIR"
tar --no-same-owner --no-same-permissions -xzf "$ARCHIVE" -C "$EXTRACT_DIR"

EXPECTED_ROOT="$EXTRACT_DIR/locoryn-$VERSION-linux-$ARCH"
if [ -f "$EXPECTED_ROOT/locoryn" ]; then
    PACKAGE_ROOT=$EXPECTED_ROOT
else
    PACKAGE_BINARY=$(find "$EXTRACT_DIR" -type f -name locoryn | sed -n '1p')
    [ -n "$PACKAGE_BINARY" ] || fail 'The release archive does not contain the Locoryn executable.'
    PACKAGE_ROOT=$(dirname "$PACKAGE_BINARY")
fi

[ -f "$PACKAGE_ROOT/locoryn" ] && [ ! -L "$PACKAGE_ROOT/locoryn" ] \
    || fail 'The Locoryn executable is missing or is an unsafe symbolic link.'
[ -f "$PACKAGE_ROOT/assets/icon-transparent.png" ] && [ ! -L "$PACKAGE_ROOT/assets/icon-transparent.png" ] \
    || fail 'The release archive is missing a safe rounded application icon.'
[ -d "$PACKAGE_ROOT/config" ] || fail 'The release archive is missing its config directory.'

if [ -L "$LAUNCHER" ]; then
    EXISTING_TARGET=$(readlink "$LAUNCHER" 2>/dev/null || true)
    [ "$EXISTING_TARGET" = "$INSTALL_DIR/locoryn" ] \
        || fail "$LAUNCHER is a symlink not owned by this installer. Move it and try again."
elif [ -e "$LAUNCHER" ]; then
    fail "$LAUNCHER already exists and is not owned by this installer. Move it and try again."
fi

install -d -m 755 "$INSTALL_DIR" "$BIN_DIR" "$APPLICATIONS_DIR" "$ICON_DIR"
cp -R "$PACKAGE_ROOT"/. "$INSTALL_DIR"/
chmod 755 "$INSTALL_DIR/locoryn"
printf '%s\n' "$RELEASE_TAG" > "$INSTALL_DIR/.installed-release"

if [ -f "$BUNDLE_DIR/uninstall-linux.sh" ]; then
    install -m 755 "$BUNDLE_DIR/uninstall-linux.sh" "$INSTALL_DIR/uninstall-linux.sh"
fi

rm -f "$LAUNCHER"
ln -s "$INSTALL_DIR/locoryn" "$LAUNCHER"
install -m 644 "$PACKAGE_ROOT/assets/icon-transparent.png" "$ICON_FILE"

DESKTOP_EXEC=$(printf '%s' "$LAUNCHER" | sed 's/\\/\\\\/g; s/"/\\"/g')
SED_REPLACEMENT=$(printf '%s' "$DESKTOP_EXEC" | sed 's/[\\&|]/\\&/g')
sed "s|@EXEC@|$SED_REPLACEMENT|g" "$DESKTOP_TEMPLATE" > "$TEMP_DIR/$APP_ID.desktop"
install -m 644 "$TEMP_DIR/$APP_ID.desktop" "$DESKTOP_FILE"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
fi

say ""
say "Locoryn $VERSION is installed."
say 'Launch it from your desktop environment or run:'
say "  $LAUNCHER"
case ":${PATH:-}:" in
    *":$BIN_DIR:"*) ;;
    *) say "Add $BIN_DIR to PATH if you want to run 'locoryn' directly in a terminal." ;;
esac
