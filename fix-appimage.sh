#!/bin/bash
# Fix AppImage EGL/Wayland crash (EGL_BAD_PARAMETER)
# Wraps AppRun with WEBKIT_DISABLE_DMABUF_RENDERER=1 to work around
# WebKitGTK DMABUF bug inside the AppImage sandbox.
#
# Usage: ./fix-appimage.sh <AppImage file> [output file]
#   If output is omitted, the input file is replaced in-place.

set -euo pipefail

APPIMAGE="$1"
OUTPUT="${2:-$APPIMAGE}"

if [ -z "$APPIMAGE" ] || [ ! -f "$APPIMAGE" ]; then
    echo "Usage: $0 <AppImage file> [output file]"
    exit 1
fi

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT

echo ">>> Extracting $APPIMAGE..."
chmod +x "$APPIMAGE"
"$APPIMAGE" --appimage-extract >/dev/null 2>&1
mv squashfs-root "$WORKDIR/"

APPRUN_REAL="$WORKDIR/squashfs-root/AppRun.real"
cp "$WORKDIR/squashfs-root/AppRun" "$APPRUN_REAL"

cat > "$WORKDIR/squashfs-root/AppRun" << 'WRAPPER'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}

# Fix WebKitGTK EGL crash inside AppImage sandbox
export WEBKIT_DISABLE_DMABUF_RENDERER=1

# Propagate schema/data dirs for GLib/GTK
export GSETTINGS_SCHEMA_DIR="$HERE/usr/share/glib-2.0/schemas:${GSETTINGS_SCHEMA_DIR:-}"
export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

exec "$HERE/AppRun.real" "$@"
WRAPPER
chmod +x "$WORKDIR/squashfs-root/AppRun"

# Fix absolute symlinks (breaks inside AppImage)
cd "$WORKDIR/squashfs-root"
DESKTOP_FILE=$(find . -maxdepth 1 -name "*.desktop" | head -1)
if [ -n "$DESKTOP_FILE" ] && [ -L "$DESKTOP_FILE" ]; then
    TARGET=$(readlink "$DESKTOP_FILE")
    rm "$DESKTOP_FILE"
    # Make relative: strip the absolute prefix
    RELATIVE="${TARGET#*squashfs-root/}"
    [ "$RELATIVE" = "$TARGET" ] && RELATIVE="usr/share/applications/$(basename "$TARGET")"
    ln -s "$RELATIVE" "$DESKTOP_FILE"
fi
cd - >/dev/null

echo ">>> Repacking AppImage..."
if ! command -v appimagetool &>/dev/null; then
    APPIMAGETOOL="$WORKDIR/appimagetool"
    wget -q "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
        -O "$APPIMAGETOOL"
    chmod +x "$APPIMAGETOOL"
else
    APPIMAGETOOL="appimagetool"
fi

[ "$OUTPUT" != "$APPIMAGE" ] && rm -f "$OUTPUT"
ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$WORKDIR/squashfs-root" "$OUTPUT" 2>&1 | tail -3

echo ">>> Done: $OUTPUT"
