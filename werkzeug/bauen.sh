#!/bin/bash
# Querencia Dokumente: fmt, Clippy, Tests und Release-Build hinter dem
# gemeinsamen Build-Lock (nice 10, 4 Jobs, offline, eigener TMPDIR).
# Der Release-Build läuft mit --remap-path-prefix (Repo → /usr/src/…,
# Cargo-Home → /usr/share/cargo, wie querencia-packaging), damit keine
# Home-Pfade im Binary stehen; geprüft wird das am Ende mit strings.
#   bauen.sh            alles
#   bauen.sh pruefen    nur fmt, Clippy, Tests
#   bauen.sh release    nur Release-Build
set -uo pipefail
ART=${1:-alles}
R=$(cd "$(dirname "$0")/.." && pwd)
L=$R/nachweis/bau; mkdir -p "$L"; LOG=$L/$ART.log
LOCK=$HOME/.local/state/querencia/bau.lock
CH=${CARGO_HOME:-$HOME/.cargo}
mkdir -p "$R/tmp"
export CARGO_BUILD_JOBS=4 TMPDIR=$R/tmp ART CH R
: >"$LOG"
df -h / | tail -1 | tee -a "$LOG"
echo "== $ART $(git -C "$R" log -1 --format='%h %s' 2>/dev/null) (Arbeitsbaum: $(git -C "$R" status --porcelain | wc -l) Änderungen): warte auf Build-Lock $(date +%T)" | tee -a "$LOG"
(cd "$R" && flock "$LOCK" bash -c '
    echo "Build-Lock erhalten $(date +%T)"
    set -x
    if [ "$ART" != release ]; then
      cargo fmt --all --check &&
      env -u WAYLAND_DISPLAY -u DISPLAY DBUS_SESSION_BUS_ADDRESS=disabled: nice -n 10 cargo clippy --all-targets --offline --locked -- -D warnings &&
      env -u WAYLAND_DISPLAY -u DISPLAY DBUS_SESSION_BUS_ADDRESS=disabled: nice -n 10 cargo test --offline --locked || exit 1
    fi
    if [ "$ART" != pruefen ]; then
      env -u WAYLAND_DISPLAY -u DISPLAY \
        RUSTFLAGS="--remap-path-prefix=$R=/usr/src/querencia-dokumente --remap-path-prefix=$CH=/usr/share/cargo" \
        CARGO_TARGET_DIR="$R/target/remap" \
        nice -n 10 cargo build --release --offline --locked || exit 1
    fi
') >>"$LOG" 2>&1
RC=$?
echo "== $ART: fertig rc=$RC $(date +%T)" | tee -a "$LOG"
B=$R/target/remap/release/querencia-dokumente
if [ "$ART" != pruefen ] && [ -f "$B" ]; then
    sha256sum "$B" | tee "$L/release-sha256.txt"
    echo "Home-Pfade im Binary: $(strings -a "$B" | grep -c "$HOME")" | tee -a "$LOG"
fi
df -h / | tail -1 | tee -a "$LOG"
exit $RC
