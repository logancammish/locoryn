#!/bin/sh
set -eu

SCRIPT_PATH=$0
case "$SCRIPT_PATH" in
    /*) ;;
    *) SCRIPT_PATH=$PWD/$SCRIPT_PATH ;;
esac

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$SCRIPT_PATH")" && pwd)
exec sh "$SCRIPT_DIR/linux_installations/install-linux.sh" "$@"
