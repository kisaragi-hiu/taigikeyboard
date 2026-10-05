#!/usr/bin/env bash
# Renders cheatsheet.html to cheatsheet/*.png at 2x, one picture per desktop
# family. Needs Google Chrome; run after shots/ has been regenerated, since
# the cheatsheet embeds three of those pictures.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"

render() {
    "$CHROME" --headless=new --hide-scrollbars --force-device-scale-factor=2 \
        --window-size=1600,1000 --allow-file-access-from-files \
        --virtual-time-budget=4000 --screenshot="cheatsheet/$1" \
        "file://$PWD/cheatsheet.html$2" 2>/dev/null
    echo "wrote cheatsheet/$1"
}

mkdir -p cheatsheet
render taigikeyboard-desktop-macos.png ""
render taigikeyboard-desktop-windows-linux.png "?os=pc"
