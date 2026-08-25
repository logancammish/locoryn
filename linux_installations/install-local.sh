#!/bin/sh
set -eu

APP_ID=io.github.logancammish.locoryn
ICON_ID=$APP_ID-transparent

SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
    /*) ;;
    *) SCRIPT_PATH=$PWD/$SCRIPT_PATH ;;
esac
BUNDLE_DIR=$(CDPATH= cd "$(dirname "$SCRIPT_PATH")" && pwd)
PAYLOAD_DIR=${LOCORYN_PAYLOAD_DIR:-"$BUNDLE_DIR/payload"}
DESKTOP_TEMPLATE="$BUNDLE_DIR/$APP_ID.desktop.in"

fail() {
    printf 'Error: %s\n' "$*" >&2
    exit 1
}

[ "$(uname -s 2>/dev/null || true)" = Linux ] || fail 'This installer only supports Linux.'
[ -n "${HOME:-}" ] || fail 'HOME is not set.'
[ -f "$PAYLOAD_DIR/locoryn" ] || fail "Missing application binary: $PAYLOAD_DIR/locoryn"
[ -f "$PAYLOAD_DIR/assets/icon-transparent.png" ] || fail 'The rounded application icon is missing from the payload.'
[ -d "$PAYLOAD_DIR/config" ] || fail 'The config directory is missing from the payload.'
[ -f "$DESKTOP_TEMPLATE" ] || fail "Missing desktop template: $DESKTOP_TEMPLATE"

DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
INSTALL_DIR=${LOCORYN_INSTALL_DIR:-"$HOME/.local/lib/locoryn"}
BIN_DIR=${LOCORYN_BIN_DIR:-"$HOME/.local/bin"}
APPLICATIONS_DIR="$DATA_HOME/applications"
ICON_DIR="$DATA_HOME/icons/hicolor/512x512/apps"
DESKTOP_FILE="$APPLICATIONS_DIR/$APP_ID.desktop"
ICON_FILE="$ICON_DIR/$ICON_ID.png"
LEGACY_ICON_FILE="$ICON_DIR/$APP_ID.png"
LAUNCHER="$BIN_DIR/locoryn"

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
        || fail "$INSTALL_DIR is not empty and was not created by Locoryn's installer."
fi

if [ -L "$LAUNCHER" ]; then
    EXISTING_TARGET=$(readlink "$LAUNCHER" 2>/dev/null || true)
    [ "$EXISTING_TARGET" = "$INSTALL_DIR/locoryn" ] \
        || fail "$LAUNCHER is a link not owned by Locoryn's installer."
elif [ -e "$LAUNCHER" ]; then
    fail "$LAUNCHER already exists and is not owned by Locoryn's installer."
fi

printf '%s\n' 'Installing Locoryn for the current user...'
install -d -m 755 "$INSTALL_DIR" "$BIN_DIR" "$APPLICATIONS_DIR" "$ICON_DIR"
cp -R "$PAYLOAD_DIR"/. "$INSTALL_DIR"/
chmod 755 "$INSTALL_DIR/locoryn"

VERSION=local-build
if [ -f "$PAYLOAD_DIR/VERSION" ]; then
    VERSION=$(sed -n '1p' "$PAYLOAD_DIR/VERSION")
fi
printf '%s\n' "$VERSION" > "$INSTALL_DIR/.installed-release"

if [ -f "$BUNDLE_DIR/uninstall-linux.sh" ]; then
    install -m 755 "$BUNDLE_DIR/uninstall-linux.sh" "$INSTALL_DIR/uninstall-linux.sh"
fi

rm -f "$LAUNCHER"
ln -s "$INSTALL_DIR/locoryn" "$LAUNCHER"
install -m 644 "$PAYLOAD_DIR/assets/icon-transparent.png" "$ICON_FILE"
rm -f "$LEGACY_ICON_FILE"

DESKTOP_EXEC=$(printf '%s' "$INSTALL_DIR/locoryn" | sed 's/\\/\\\\/g; s/"/\\"/g')
SED_REPLACEMENT=$(printf '%s' "$DESKTOP_EXEC" | sed 's/[\\&|]/\\&/g')
sed "s|@EXEC@|$SED_REPLACEMENT|g" "$DESKTOP_TEMPLATE" > "$DESKTOP_FILE"
chmod 644 "$DESKTOP_FILE"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental >/dev/null 2>&1 || true
fi

printf '\nLocoryn %s is installed.\n' "$VERSION"
printf 'Launch it from your application menu or run: %s\n' "$LAUNCHER"
