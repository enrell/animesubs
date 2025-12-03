#!/bin/bash
# Fix AppImage EGL conflicts for Wayland
# Usage: ./fix-appimage.sh path/to/app.AppImage

set -e

APPIMAGE="$1"
if [ -z "$APPIMAGE" ] || [ ! -f "$APPIMAGE" ]; then
    echo "Usage: $0 <AppImage file>"
    exit 1
fi

echo "Extracting $APPIMAGE..."
chmod +x "$APPIMAGE"
"$APPIMAGE" --appimage-extract

echo "Removing conflicting EGL/Wayland libraries..."
cd squashfs-root/usr/lib
rm -fv libwayland-egl.so* libEGL.so* libEGL_mesa.so* libGLX.so* libGLX_mesa.so* 2>/dev/null || true
rm -fv libwayland-client.so* libwayland-cursor.so* libwayland-server.so* 2>/dev/null || true
cd ../../..

echo "Repacking AppImage..."
if ! command -v appimagetool &> /dev/null; then
    wget -q https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage -O /tmp/appimagetool
    chmod +x /tmp/appimagetool
    APPIMAGETOOL="/tmp/appimagetool"
else
    APPIMAGETOOL="appimagetool"
fi

rm "$APPIMAGE"
ARCH=x86_64 "$APPIMAGETOOL" squashfs-root "$APPIMAGE"
rm -rf squashfs-root

echo "Done! Fixed AppImage: $APPIMAGE"
