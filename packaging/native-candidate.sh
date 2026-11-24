#!/usr/bin/env bash
set -euo pipefail

binary=$(realpath "$1")
output=$(realpath -m "$2")
appimagetool=$(realpath "$3")
version=$4
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ -x "$binary" && -x "$appimagetool" ]]
root=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'python3 -c "import shutil, sys; shutil.rmtree(sys.argv[1])" "$scratch"' EXIT
mkdir -p "$output"

# All packages use these same executable, desktop entry and icon bytes.
payload="$scratch/payload"
install -Dm755 "$binary" "$payload/usr/bin/pinned"
install -Dm644 "$root/packaging/pin.desktop" "$payload/usr/share/applications/pin.desktop"
install -Dm644 "$root/logo/pin-256.png" "$payload/usr/share/icons/hicolor/256x256/apps/pin.png"
install -Dm644 "$root/LICENSE" "$payload/usr/share/doc/pinned/copyright"
install -Dm644 "$root/native/icons/LICENSE" "$payload/usr/share/doc/pinned/LUCIDE-LICENSE"

deb="$scratch/deb"
cp -a "$payload" "$deb"
mkdir "$deb/DEBIAN"
cat > "$deb/DEBIAN/control" <<META
Package: pinned
Version: $version
Architecture: amd64
Maintainer: Masih Moafi <hi@masihmoafi.com>
Section: utils
Priority: optional
Depends: libc6 (>= 2.39), libgcc-s1, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrandr2, libxkbcommon-x11-0
Description: Always-on-top priority note
 Edit ranked tasks, subtasks and dates in a plain Markdown file.
META
dpkg-deb --root-owner-group --build "$deb" "$output/pinned_${version}_amd64.deb"
dpkg-deb -x "$output/pinned_${version}_amd64.deb" "$scratch/deb-check"
cmp "$binary" "$scratch/deb-check/usr/bin/pinned"

appdir="$scratch/Pin.AppDir"
cp -a "$payload" "$appdir"
ln -s usr/share/applications/pin.desktop "$appdir/pin.desktop"
ln -s usr/share/icons/hicolor/256x256/apps/pin.png "$appdir/pin.png"
cat > "$appdir/AppRun" <<'LAUNCH'
#!/bin/sh
exec "$(dirname "$0")/usr/bin/pinned" "$@"
LAUNCH
chmod 755 "$appdir/AppRun"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$appimagetool" --no-appstream "$appdir" "$output/Pin-$version.AppImage"
chmod 755 "$output/Pin-$version.AppImage"
(cd "$scratch" && "$output/Pin-$version.AppImage" --appimage-extract > /dev/null)
cmp "$binary" "$scratch/squashfs-root/usr/bin/pinned"

snap="$scratch/snap"
cp -a "$payload" "$snap"
# Iced opens X11 libraries dynamically, so they do not appear in ldd output
# for the executable. Stage them against the same Ubuntu release as core24.
(
  cd "$scratch"
  apt-get download libx11-6 libx11-xcb1 libxau6 libxcb1 libxcb-xkb1 libxcursor1 \
    libxdmcp6 libxext6 libxfixes3 libxi6 libxinerama1 libxkbcommon0 \
    libxkbcommon-x11-0 libxrandr2 libxrender1
  for dependency in ./*.deb; do
    dpkg-deb -x "$dependency" "$snap"
  done
)
mkdir -p "$snap/meta/gui"
sed "s|^Icon=pin$|Icon=\${SNAP}/meta/gui/icon.png|" "$root/packaging/pin.desktop" > "$snap/meta/gui/pinned.desktop"
install -m644 "$root/logo/pin-256.png" "$snap/meta/gui/icon.png"
cat > "$snap/meta/snap.yaml" <<META
name: pinned
version: '$version'
summary: Always-on-top priority note
description: Edit ranked tasks, subtasks and dates in a plain Markdown file.
base: core24
grade: stable
confinement: strict
architectures: [amd64]
apps:
  pinned:
    command: usr/bin/pinned
    plugs: [home, desktop, x11]
    environment:
      LD_LIBRARY_PATH: \$SNAP/usr/lib/x86_64-linux-gnu
META
snap pack "$snap" "$output"
unsquashfs -cat "$output/pinned_${version}_amd64.snap" usr/bin/pinned > "$scratch/snap-binary"
cmp "$binary" "$scratch/snap-binary"
desktop-file-validate "$payload/usr/share/applications/pin.desktop"
(cd "$output" && sha256sum ./*.deb ./*.AppImage ./*.snap > SHA256SUMS && sha256sum -c SHA256SUMS)
