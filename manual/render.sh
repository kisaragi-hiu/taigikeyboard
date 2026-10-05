#!/usr/bin/env bash
# Renders the manual's generated pictures at 2x with headless Chrome:
#   keyboards.py    -> keyboards/*.png   (one keyboard per topic and platform)
#   cheatsheet.html -> cheatsheet/*.png  (one picture per desktop family)
# Run after shots/ has been regenerated, since the cheatsheet embeds three of
# those pictures.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"

# shoot <output.png> <width> <height> <url>
shoot() {
    "$CHROME" --headless=new --hide-scrollbars --force-device-scale-factor=2 \
        --window-size="$2,$3" --allow-file-access-from-files \
        --virtual-time-budget=4000 --screenshot="$1" "$4" 2>/dev/null
    echo "wrote $1"
}

svgs="$(mktemp -d)"
trap 'rm -rf "$svgs"' EXIT
python3 keyboards.py "$svgs"
mkdir -p keyboards
for svg in "$svgs"/*.svg; do
    size="$(sed -E 's/^<svg[^>]* width="([0-9]+)" height="([0-9]+)".*/\1 \2/' "$svg" | head -1)"
    # shellcheck disable=SC2086
    shoot "keyboards/$(basename "$svg" .svg).png" $size "file://$svg"
done

mkdir -p cheatsheet
shoot cheatsheet/taigikeyboard-desktop-macos.png 1600 1000 "file://$PWD/cheatsheet.html"
shoot cheatsheet/taigikeyboard-desktop-windows-linux.png 1600 1000 "file://$PWD/cheatsheet.html?os=pc"
