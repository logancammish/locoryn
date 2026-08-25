#!/bin/sh
set -eu

APP_ID=io.github.logancammish.locoryn
ICON_ID=$APP_ID-transparent
DATA_HOME=${XDG_DATA_HOME:-"$HOME/.local/share"}
INSTALL_DIR=${LOCORYN_INSTALL_DIR:-"$HOME/.local/lib/locoryn"}
BIN_DIR=${LOCORYN_BIN_DIR:-"$HOME/.local/bin"}
DESKTOP_FILE="$DATA_HOME/applications/$APP_ID.desktop"
ICON_FILE="$DATA_HOME/icons/hicolor/512x512/apps/$ICON_ID.png"
LEGACY_ICON_FILE="$DATA_HOME/icons/hicolor/512x512/apps/$APP_ID.png"
LAUNCHER="$BIN_DIR/locoryn"

case "$INSTALL_DIR" in
    "$HOME"|"$HOME/"|"$HOME/.local"|"$HOME/.local/"|"$HOME/.local/lib"|"$HOME/.local/lib/"|/|"")
        printf '%s\n' "Refusing to remove an unsafe installation path: $INSTALL_DIR" >&2
        exit 1
        ;;
    "$HOME"/*) ;;
    *)
        printf '%s\n' "Refusing to remove an installation outside your home directory: $INSTALL_DIR" >&2
        exit 1
        ;;
esac
case "$BIN_DIR" in
    "$HOME"|"$HOME/"|/|"")
        printf '%s\n' "Refusing to use an unsafe command directory: $BIN_DIR" >&2
        exit 1
        ;;
    "$HOME"/*) ;;
    *)
        printf '%s\n' "Refusing to use a command directory outside your home: $BIN_DIR" >&2
        exit 1
        ;;
esac

if [ ! -f "$INSTALL_DIR/.installed-release" ]; then
    printf '%s\n' "Refusing to uninstall an application directory without the installer marker: $INSTALL_DIR" >&2
    exit 1
fi

if [ -L "$LAUNCHER" ] && [ "$(readlink "$LAUNCHER" 2>/dev/null || true)" = "$INSTALL_DIR/locoryn" ]; then
    rm -f "$LAUNCHER"
elif [ -e "$LAUNCHER" ]; then
    printf '%s\n' "Leaving launcher not owned by this installer in place: $LAUNCHER" >&2
fi
rm -f "$DESKTOP_FILE" "$ICON_FILE" "$LEGACY_ICON_FILE"
rm -rf "$INSTALL_DIR"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DATA_HOME/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATA_HOME/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
elif command -v kbuildsycoca5 >/dev/null 2>&1; then
    kbuildsycoca5 --noincremental >/dev/null 2>&1 || true
fi

printf '%s\n' "Locoryn was uninstalled."
printf '%s\n' "Your chats and settings in $DATA_HOME/locoryn were left in place."
