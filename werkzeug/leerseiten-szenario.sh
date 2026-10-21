#!/bin/bash
# Dokumente 0.1.3: Leerseiten-Probe (Befund 21.10.2026: Seite in der
# Hauptansicht weiß, Miniatur mit Inhalt). Läuft in der isolierten Sitzung
# (isoliert.py, über lauf.sh mit SZENARIO=leerseiten-szenario.sh).
# Umgebung aus $QD_PROFILE/env.sh; dazu QD_PDF (das zu prüfende PDF, nur
# gelesen), QD_GSK (GSK_RENDERER, leer = GTK-Standard wie live),
# QD_TEILE (Auswahl aus: sprung100 bildab mausrad z50 z150 ganz leiste),
# QD_MAX=0 (nicht maximieren), QD_DUNKEL=1 (dunkle Fassung des Themas), QD_PDF_NEU (zweite Fassung: dann wird eine
# Kopie von QD_PDF im Profil geöffnet; Teil „umschreiben“ überschreibt sie
# am selben Inode mit QD_PDF_NEU, Teil „loeschen“ löscht sie).
# GSK_DEBUG=full-redraw wie in der Sitzung (labwc-environment, QD-070).
# Jedes Bild: <nr>-<teil>-<seite|schritt>-<spr>-<res>.png; ausgewertet mit
# werkzeug/leerseiten.py.
set -u
. "$QD_PROFILE/env.sh"
Q=$W/bin/qeingabe; QA=$W/bin/qausgabe
L=$QD_PROFILE/logs
mkdir -p "$OUT"
"$QA" "$RES" >>"$L/szenario.log" 2>&1; sleep 1
B=${RES%x*}; H=${RES#*x}
if [ "$B" -gt 1500 ]; then WT=1400; else WT=800; fi
case $SPR in
de) FIT=g ;;
*) FIT=b ;;
esac
export GSK_DEBUG=full-redraw RUST_BACKTRACE=1
if [ -n "${QD_GSK:-}" ]; then export GSK_RENDERER=$QD_GSK; else unset GSK_RENDERER; fi
if [ -n "${QD_PDF_NEU:-}" ]; then
    KOPIE=$QD_PROFILE/home/Dokumente/dokument.pdf
    cp "$QD_PDF" "$KOPIE"; QD_PDF=$KOPIE
fi
N=$(python3 -I "$W/pdfseiten.py" "$QD_PDF")
# QD_DUNKEL=1: dunkle Fassung des Themas über gtk-interface-color-scheme im
# settings.ini des Profils (statt Portal; Dokumente meldet es beim Start)
if [ "${QD_DUNKEL:-}" = 1 ]; then
    echo "gtk-interface-color-scheme=dark" >>"$QD_PROFILE/config/gtk-4.0/settings.ini"
fi
TEILE=${QD_TEILE:-sprung100 bildab mausrad z50 z150 ganz leiste}
echo "LANG=$LANG LANGUAGE=$LANGUAGE BIN=$BIN GSK_RENDERER=${GSK_RENDERER:-} PDF=$QD_PDF ($N Seiten) TEILE=$TEILE" >>"$L/szenario.log"
"$BIN" "$QD_PDF" >"$L/app.log" 2>&1 &
PID=$!
echo "app pid $PID $(readlink /proc/$PID/exe)" >>"$L/szenario.log"
sleep 3
. "$W/eingabe.sh"
n=0
shot() { n=$((n + 1)); local f; f=$(printf '%s/%03d-%s-%s-%s.png' "$OUT" "$n" "$1" "$SPR" "$RES"); grim "$f"; echo "$(date +%T) $f" >>"$L/szenario.log"; }
lebt() { kill -0 "$PID" 2>/dev/null && echo lebt || echo beendet; }
schritt() { local name=$1; shift; q "$@" "wait $WT"; shot "$name"; echo "$(date +%T) $name: $(lebt)" >>"$L/ablauf.log"; }
# Alle Seiten über das Seitenfeld (Strg+L) anspringen
seiten() {
    local teil=$1 p
    for p in $(seq 1 "$N"); do
        schritt "$teil-s$(printf %02d "$p")" "chord ctrl l" "wait 200" "type $p" "wait 200" "key Return"
    done
}
hat() { case " $TEILE " in *" $1 "*) return 0 ;; esac; return 1; }

q "size $B $H" "move $((B / 2)) $((H / 2))" "wait 300"
shot start
# Maximieren wie in der Sitzung (Doppelklick auf die Titelleiste; das
# Fenster öffnet 900×728 in der Mitte), QD_MAX=0 lässt es so
if [ "${QD_MAX:-1}" = 1 ]; then
    schritt max "move $((B / 2)) $(((H - 728) / 2 + 13))" "dclick" "wait 800"
fi
schritt z100 "chord ctrl 0"
hat sprung100 && seiten sprung100
if hat bildab; then
    schritt bildab-s00 "key Home"
    for i in $(seq 1 $((N * 3 / 2))); do schritt "bildab-b$(printf %02d "$i")" "key Page_Down"; done
fi
if hat mausrad; then
    schritt mausrad-s00 "key Home" "move $((B * 60 / 100)) $((H / 2))"
    for i in $(seq 1 $((N * 3 / 2))); do schritt "mausrad-r$(printf %02d "$i")" "scroll 6"; done
fi
if hat z50; then
    schritt z50 "chord ctrl 0" "wait 300" "chord ctrl minus" "wait 300" "chord ctrl minus" "wait 300" "chord ctrl minus"
    seiten z50
fi
if hat z150; then
    schritt z150 "chord ctrl 0" "wait 300" "chord ctrl plus" "wait 300" "chord ctrl plus"
    seiten z150
fi
if hat ganz; then
    schritt ganz "key F10" "wait 400" "key Right" "wait 300" "key Right" "wait 300" "key Down" "wait 300" "key $FIT"
    seiten ganz
fi
if hat umschreiben; then
    # Miniaturen und die ersten Seiten aus der ersten Fassung, dann die Datei
    # wie WeasyPrint am selben Inode neu schreiben (Befund 21.10.2026)
    schritt umschr-vorher "chord ctrl 0" "wait 300" "key Home" "wait 1500"
    cat "$QD_PDF_NEU" >"$QD_PDF"
    echo "$(date +%T) überschrieben: $(stat -c '%i %s' "$QD_PDF")" >>"$L/ablauf.log"
    schritt umschr-danach "wait 1500"
    N=$(python3 -I "$W/pdfseiten.py" "$QD_PDF")
    seiten umschr
fi
if hat loeschen; then
    schritt loesch-vorher "chord ctrl l" "wait 200" "type 5" "wait 200" "key Return"
    rm -f -- "$QD_PDF"
    echo "$(date +%T) gelöscht" >>"$L/ablauf.log"
    schritt loesch-danach "wait 1500"
    seiten loesch
fi
if hat leiste; then
    # Seitenleiste: erste Miniatur anklicken, dann mit ↓ von Miniatur zu
    # Miniatur (jede Auswahl springt in der Hauptansicht zur Seite)
    schritt leiste-s01 "chord ctrl 0" "wait 300" "key Home" "wait 500" "move 67 130" "click"
    for p in $(seq 2 "$N"); do schritt "leiste-s$(printf %02d "$p")" "key Down"; done
fi
q "chord ctrl q"
for i in $(seq 1 100); do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$PID" 2>/dev/null; then
    echo "ENDE Strg+Q: läuft noch" >"$L/status.txt"
    kill "$PID" 2>/dev/null
else
    wait "$PID"; echo "ENDE rc=$? Bilder=$n" >"$L/status.txt"
fi
# Ende melden: isoliert.py wartet sonst bis zum Zeitlimit
touch "$L/FERTIG"
