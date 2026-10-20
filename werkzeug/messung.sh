#!/bin/bash
# Dokumente 0.1.0: Speicher und Zeit mit gross-500.pdf (500 Seiten A4, Text
# und Vektorgrafik), isoliert wie szenario.sh. Zeiten aus dem Diagnoseprotokoll
# (QUERENCIA_DOKUMENTE_DEBUG), Speicher aus /proc/<pid>/status (VmRSS, VmHWM).
# Ergebnis: $OUT/messung.txt, Bildschirmfotos.
set -u
. "$QD_PROFILE/env.sh"
Q=$W/bin/qeingabe; QA=$W/bin/qausgabe
L=$QD_PROFILE/logs
M=$OUT/messung.txt
mkdir -p "$OUT"
"$QA" "$RES" >>"$L/szenario.log" 2>&1; sleep 1
B=${RES%x*}; H=${RES#*x}
export GSK_RENDERER=cairo RUST_BACKTRACE=1
T0=$(date +%s.%N)
"$BIN" "$T/gross-500.pdf" >"$L/app.log" 2>&1 &
PID=$!
. "$W/eingabe.sh"
n=0
shot() { n=$((n + 1)); grim "$(printf '%s/%02d-%s-%s-%s.png' "$OUT" "$n" "$1" "$SPR" "$RES")"; }
mem() { echo "$1: $(grep -E '^(VmRSS|VmHWM)' "/proc/$PID/status" | tr -s ' \t' ' ' | tr '\n' ' ')$(grep -c '^querencia-dokumente: Seite' "$L/app.log") Seitenbilder" >>"$M"; }
since() { echo "$(echo "$(date +%s.%N) - $1" | bc | cut -c1-6) s"; }
# warten, bis das Protokoll eine Zeile mit $1 enthält (höchstens $2 s)
warte_auf() { local i; for i in $(seq 1 $(($2 * 10))); do grep -q "$1" "$L/app.log" && return 0; sleep 0.1; done; return 1; }

echo "Datei: gross-500.pdf $(stat -c %s "$T/gross-500.pdf") Byte, Auflösung $RES, Sprache $SPR" >"$M"
warte_auf "erste Seite nach" 30
echo "Start bis erste Seite (Wanduhr ab Prozessstart): $(since "$T0")" >>"$M"
grep -E 'geöffnet in|erste Seite nach' "$L/app.log" >>"$M"
q "size $B $H" "move $((B / 2)) $((H / 2))" "wait 1500"
mem "nach dem Start"
shot start
# Sprung ans Ende und zurück
T1=$(date +%s.%N); q "key End" "wait 1000"; mem "Ende (Seite 500)"; shot ende
q "key Home" "wait 800"
# 80 × Bild ab, ohne Pause dazwischen (Eingabe schneller als das Rendern)
T1=$(date +%s.%N)
for i in $(seq 1 80); do printf 'key Page_Down\n' >&"${QE[1]}"; done
q "wait 50"
echo "80 × Bild ab abgeschickt in $(since "$T1")" >>"$M"
q "wait 1500"; mem "nach 80 × Bild ab"; shot bild-ab-80
# 400 %: Strg+Plus bis zum Anschlag, dann 20 Seiten weiter
for i in $(seq 1 10); do printf 'chord ctrl plus\n' >&"${QE[1]}"; done
q "wait 2000"; mem "400 %"; shot zoom-400
for i in $(seq 1 20); do q "key Page_Down" "wait 250"; done
q "wait 1500"; mem "400 %, 20 Seiten weiter"; shot zoom-400-weiter
grep 'Cache' "$L/app.log" | tail -1 | sed 's/^/letzter Cache-Stand: /' >>"$M"
# kleinste Stufe: viele Seiten gleichzeitig sichtbar
for i in $(seq 1 14); do printf 'chord ctrl minus\n' >&"${QE[1]}"; done
q "wait 2500"; mem "kleinste Stufe"; shot zoom-klein
# Suche über alle 500 Seiten; währenddessen blättern (Oberfläche bleibt bedienbar)
q "chord ctrl 0" "wait 800" "key Home" "wait 500" "chord ctrl f" "wait 300"
T1=$(date +%s.%N)
q "type Leuchtturm"
q "wait 400"; shot suche-laeuft
q "chord ctrl Page_Down" "wait 300"; shot suche-blaettern-waehrenddessen
warte_auf "Suche über" 120
echo "Suche (Wanduhr ab Eingabe, inkl. 300 ms Pause vor dem Start): $(since "$T1")" >>"$M"
grep 'Suche über' "$L/app.log" >>"$M"
q "wait 800"; shot suche-fertig
q "key Return" "wait 800"; shot suche-weiter
mem "nach der Suche"
q "key Escape" "wait 300"
echo "Höchststand: $(grep VmHWM "/proc/$PID/status" | tr -s ' \t' ' ')" >>"$M"
q "chord ctrl q" "wait 2500"
if kill -0 "$PID" 2>/dev/null; then echo "ENDE: läuft noch" >"$L/status.txt"; else wait "$PID"; echo "ENDE: beendet, Exitcode $?" >"$L/status.txt"; fi
cat "$L/status.txt" >>"$M"
touch "$L/FERTIG"
