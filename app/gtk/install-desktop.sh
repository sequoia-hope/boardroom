#!/usr/bin/env bash
# install-desktop.sh -- put Boardroom in the desktop's app launcher, pointing
# at this checkout's build. Run again after moving the checkout; remove with
#     rm ~/.local/share/applications/boardroom.desktop
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$here/target/release/boardroom-gtk"
[[ -x "$bin" ]] || { echo "install-desktop.sh: build first (app/build.sh)" >&2; exit 1; }
dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mkdir -p "$dir"
cat > "$dir/boardroom.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Boardroom
Comment=Board projects: phases, gates, and the suite's apps
Exec=$bin
Icon=applications-engineering
Terminal=false
Categories=Development;Electronics;Engineering;
StartupWMClass=io.github.sequoia_hope.Boardroom
DESKTOP
echo "Installed $dir/boardroom.desktop"
