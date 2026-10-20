#!/bin/bash
# Dokumente 0.1.0: Wegwerfprofil für einen isolierten Lauf (headless labwc).
# Kopiert lesend: labwc rc.xml/environment der Sitzung (Runtime-Repo), hicolor
# des Benutzers (ohne 48x48) plus das Programmsymbol aus data/icons (wie es
# das Integrationsskript ablegt); Thema Querencia-Klassisch und Symbolthema
# Querencia als Link. Legt eine Ablage-Kopie unter
# <profil>/opt/dokumente/<sha16>/ an (Binary und share/locale wie die spätere
# Live-Ablage) und den Link <profil>/opt/bin/querencia-dokumente darauf.
# Test-PDFs (werkzeug/testpdf.py) liegen unter <profil>/home/Dokumente/pdf.
# gvfs ist im Wegwerf-Bus nicht aktivierbar (wsdd-Falle).
#   profil.sh <name> [ablage-quelle]   → gibt den Profilpfad aus
set -euo pipefail
R=$(cd "$(dirname "$0")/.." && pwd)
P=$R/gui-test/${1:?name}
Q=${2:-$R/integration/vorlage}
case $P in "$R"/gui-test/?*) ;; *) echo "Profil nur unter gui-test/" >&2; exit 2 ;; esac
[ -e "$P" ] && { chmod -R u+rwX "$P"; rm -rf -- "$P"; }
mkdir -p "$P"/{config/labwc,data/themes,data/icons,state,cache,logs,opt/bin,opt/dokumente,home/Dokumente}
L=${QD_LABWC_CONFIG:-$HOME/projects/querencia-runtime/session/querencia/labwc}
cp "$L/rc.xml" "$L/environment" "$P/config/labwc/"
ln -s "$HOME/.local/share/themes/Querencia-Klassisch" "$P/data/themes/Querencia-Klassisch"
cp -a "$HOME/.local/share/icons/hicolor" "$P/data/icons/"
[ -d "$HOME/.local/share/icons/Querencia" ] && ln -s "$HOME/.local/share/icons/Querencia" "$P/data/icons/Querencia"
for d in "$R"/data/icons/hicolor/*/apps; do
    mkdir -p "$P/data/icons/hicolor/${d#"$R"/data/icons/hicolor/}"
    cp "$d"/org.querencia.Dokumente.* "$P/data/icons/hicolor/${d#"$R"/data/icons/hicolor/}/"
done
if find "$P/data/icons/" -path '*48x48*' -name 'org.querencia.*' | grep -q .; then
    echo "48x48-Programmsymbol im Profil – Abbruch (GTK-4.22-Symbolabsturz)" >&2; exit 2
fi
S=$(sha256sum "$Q/querencia-dokumente" | cut -c1-16)
A=$P/opt/dokumente/$S
mkdir -p "$A/share/locale/de/LC_MESSAGES"
cp "$Q/querencia-dokumente" "$A/"
cp "$Q/share/locale/de/LC_MESSAGES/querencia-dokumente.mo" "$A/share/locale/de/LC_MESSAGES/"
chmod -R a-w "$A"
ln -s "$A/querencia-dokumente" "$P/opt/bin/querencia-dokumente"
python3 -I "$R/werkzeug/testpdf.py" "$P/home/Dokumente/pdf" >/dev/null
mkdir -p "$P/data/dbus-1/services"
for n in org.gtk.vfs.Daemon org.gtk.vfs.Metadata org.gtk.vfs.UDisks2VolumeMonitor org.gtk.vfs.GoaVolumeMonitor org.gtk.vfs.GPhoto2VolumeMonitor org.gtk.vfs.MTPVolumeMonitor org.gtk.vfs.AfcVolumeMonitor; do
    printf '[D-BUS Service]\nName=%s\nExec=/bin/false\n' "$n" >"$P/data/dbus-1/services/$n.service"
done
echo "$P"
