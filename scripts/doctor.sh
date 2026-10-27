#!/usr/bin/env bash
# doctor.sh -- check this machine has what the design flow and pcbview need.
# Exits non-zero if anything required is missing.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fail=0
ok()   { printf '  ok    %s\n' "$1"; }
bad()  { printf '  MISS  %s\n        %s\n' "$1" "$2"; fail=1; }
warn() { printf '  warn  %s\n        %s\n' "$1" "$2"; }

echo "Required"
if command -v kicad-cli >/dev/null; then
  v="$(kicad-cli version 2>/dev/null)"
  if [[ "${v%%.*}" -ge 9 ]]; then ok "kicad-cli $v"
  else bad "kicad-cli $v" "pcbview needs KiCad 9 or newer: https://www.kicad.org/download/"; fi
else
  bad "kicad-cli" "install KiCad 9+: https://www.kicad.org/download/"
fi
if python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))' 2>/dev/null; then
  ok "python $(python3 -c 'import platform; print(platform.python_version())')"
else
  bad "python 3.11+" "install a newer python3"
fi
python3 -c 'import numpy' 2>/dev/null && ok "numpy" || bad "numpy" "pip install numpy (or your distro's python3-numpy)"
if [[ -x "$root/vendor/pcbview/bin/pcbview" || -n "${PCBVIEW:-}" ]]; then
  ok "pcbview ($(git -C "$root/vendor/pcbview" describe --always 2>/dev/null || echo "\$PCBVIEW"))"
else
  bad "pcbview" "git submodule update --init"
fi
if [[ -f "$root/vendor/ultraroute/core/Cargo.toml" || -n "${ULTRAROUTE:-}" ]]; then
  ok "ultraroute ($(git -C "$root/vendor/ultraroute" describe --always 2>/dev/null || echo "\$ULTRAROUTE"))"
else
  bad "ultraroute" "git submodule update --init"
fi
if [[ -n "${ULTRAROUTE:-}" ]] || command -v cargo >/dev/null; then
  ok "cargo (builds ultraroute, the autorouter)"
else
  bad "cargo" "install Rust (https://rustup.rs): scripts/route.sh builds the autorouter with it"
fi

echo "Recommended"
python3 -c 'import pcbnew' 2>/dev/null && ok "KiCad python module (pcbnew)" \
  || warn "pcbnew" "KiCad's python module: without it the viewer falls back to the Edge.Cuts bounding box"
command -v ngspice >/dev/null && ok "ngspice" \
  || warn "ngspice" "needed for phase 8 simulations"

exit $fail
