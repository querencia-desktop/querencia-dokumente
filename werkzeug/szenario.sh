#!/bin/bash
# Dokumente 0.1.0: GUI-Prüfung in der isolierten Sitzung (isoliert.py), je Sprache.
# Umgebung aus $QD_PROFILE/env.sh: BIN, RES, OUT, SPR (de|en), T (Test-PDFs), W.
# Tasten (de-Layout), eine Mausauswahl, Bildschirmfotos; beendet mit Strg+Q.
# Belege: alle fünf Menüs und „Zuletzt geöffnet“, Blättern, Seitenfeld,
# Zoomstufen und Fit-Modi, Strg+Mausrad, Suche mit Treffern, Auswahl und
# Kopieren (wl-paste), Eigenschaften, Seitenleiste, Vollbild, Drucken in eine
# PDF-Datei im Profil, Info, Öffnen-Dialog, Fehlermeldung (kaputt.pdf),
# Passwortabfrage (falsch, dann richtig).
set -u
. "$QD_PROFILE/env.sh"
Q=$W/bin/qeingabe; QA=$W/bin/qausgabe
L=$QD_PROFILE/logs
mkdir -p "$OUT"
"$QA" "$RES" >>"$L/szenario.log" 2>&1; sleep 1
B=${RES%x*}; H=${RES#*x}
# Software-Renderer zeichnet 1920×1080 langsamer: längere Wartezeiten
if [ "$B" -gt 1500 ]; then WT=1400; else WT=800; fi
# Zugriffstasten je Sprache: Info im Menü Hilfe, Ganze Seite/Seitenbreite in Ansicht
case $SPR in
de) INFO=i; FIT=g; WIDTH=r; DRUCK=d ;;
*) INFO=a; FIT=b; WIDTH=w; DRUCK=p ;;
esac
export GSK_RENDERER=cairo RUST_BACKTRACE=1
echo "LANG=$LANG LANGUAGE=$LANGUAGE BIN=$BIN" >>"$L/szenario.log"
"$BIN" "$T/bericht.pdf" >"$L/app.log" 2>&1 &
PID=$!
echo "app pid $PID $(readlink /proc/$PID/exe)" >>"$L/szenario.log"
sleep $((WT / 160))
. "$W/eingabe.sh"
n=0
shot() { n=$((n + 1)); local f; f=$(printf '%s/%02d-%s-%s-%s.png' "$OUT" "$n" "$1" "$SPR" "$RES"); grim "$f"; echo "$(date +%T) $f" >>"$L/szenario.log"; }
lebt() { kill -0 "$PID" 2>/dev/null && echo lebt || echo beendet; }
schritt() { local name=$1; shift; q "$@" "wait $WT"; shot "$name"; echo "$(date +%T) $name: $(lebt)" >>"$L/ablauf.log"; }

q "size $B $H" "move $((B / 2)) $((H / 2))" "wait 300"
shot start
MO=$(grep -o '/[^ ]*querencia-dokumente\.mo' "/proc/$PID/maps" 2>/dev/null | head -1)
echo "katalog: ${MO:-keiner}" >>"$L/ablauf.log"
# Menüs: F10 öffnet „Datei“/„File“, → wechselt zum nächsten Menü (ein ↓
# würde „Zuletzt geöffnet“ markieren, dann öffnete → dessen Untermenü)
schritt menue-1-datei "key F10" "wait 400"
schritt menue-2-bearbeiten "key Right"
schritt menue-3-ansicht "key Right"
schritt menue-4-gehe-zu "key Right"
schritt menue-5-hilfe "key Right"
schritt menue-zu "key Escape" "wait 300" "key Escape"
# Datei → Zuletzt geöffnet (F10 öffnet „Datei“, ↓ markiert den zweiten
# Eintrag „Zuletzt geöffnet“, → öffnet das Untermenü)
schritt menue-zuletzt "key F10" "wait 400" "key Down" "wait 400" "key Right"
schritt menue-zuletzt-zu "key Escape" "wait 300" "key Escape" "wait 300" "key Escape"
# Blättern
schritt bild-ab "key Page_Down"
schritt bild-ab-2 "key Page_Down"
schritt ende "key End"
schritt anfang "key Home"
schritt pfeil-runter "key Down" "key Down" "key Down"
# Seitenfeld (Strg+L), Seite 5 ist quer
schritt seitenfeld "chord ctrl l" "wait 300" "type 5" "wait 300" "key Return"
# Zoom
schritt zoom-100 "chord ctrl 0"
schritt zoom-plus "chord ctrl plus"
schritt zoom-plus-2 "chord ctrl plus"
schritt zoom-minus "chord ctrl minus"
schritt zoom-ganze-seite "key F10" "wait 400" "key Right" "wait 300" "key Right" "wait 300" "key Down" "wait 300" "key $FIT"
schritt zoom-mausrad "move $((B / 2)) $((H / 2))" "keydown Control_L" "scroll -2" "keyup Control_L"
schritt zoom-seitenbreite "key F10" "wait 400" "key Right" "wait 300" "key Right" "wait 300" "key Down" "wait 300" "key $WIDTH"
# Suche: Zwetschgenkuchen steht auf Seite 3 und 9
schritt suche-leiste "key Home" "wait 300" "chord ctrl f"
schritt suche-treffer "type Zwetschgenkuchen" "wait 1500"
schritt suche-weiter "key Return"
schritt suche-zurueck "chord shift Return"
schritt suche-viele "chord ctrl a" "type Querencia" "wait 1500"
schritt suche-f3 "key Escape" "wait 300" "key F3"
schritt suche-zu "chord ctrl f" "wait 300" "key Escape"
# Auswahl mit der Maus und Kopieren
schritt auswahl "key Home" "wait 500" "move $((B * 40 / 100)) $((H * 40 / 100))" "press" "glide $((B * 62 / 100)) $((H * 52 / 100)) 400" "release"
q "chord ctrl c" "wait 500"
wl-paste -n >"$OUT/zwischenablage-auswahl.txt" 2>>"$L/szenario.log"
echo "zwischenablage auswahl: $(wc -c <"$OUT/zwischenablage-auswahl.txt") Byte" >>"$L/ablauf.log"
schritt alles "chord ctrl a"
q "chord ctrl c" "wait 800"
wl-paste -n >"$OUT/zwischenablage-alles.txt" 2>>"$L/szenario.log"
echo "zwischenablage alles: $(wc -c <"$OUT/zwischenablage-alles.txt") Byte, $(grep -c 'Kapitel' "$OUT/zwischenablage-alles.txt") Kapitel-Zeilen" >>"$L/ablauf.log"
q "key Escape"
# Eigenschaften
schritt eigenschaften "chord alt Return" "wait 600"
schritt eigenschaften-zu "key Escape"
# Seitenleiste aus und ein
schritt seitenleiste-aus "key F9"
schritt seitenleiste-ein "key F9"
# Vollbild
schritt vollbild "key F11"
schritt vollbild-escape "key Escape"
# Info über das Menü (F10 → ← = Hilfe → ↓ → Zugriffstaste)
schritt info "key F10" "wait 400" "key Left" "wait 400" "key Down" "wait 300" "key $INFO" "wait 800"
schritt info-zu "key Escape"
# Öffnen-Dialog
schritt oeffnen "chord ctrl o" "wait 1500"
schritt oeffnen-zu "key Escape" "wait 800"
# Drucken in eine Datei: Druckdialog (ohne CUPS-Drucker ist „In Datei drucken“
# vorgewählt), Eingabetaste druckt nach $HOME im Profil
touch "$L/vor-druck"
schritt drucken-dialog "chord ctrl p" "wait 2500"
# Fokus steht auf dem Reiter „Allgemein“: Tab in die Druckerliste, ↓ wählt
# „In Datei drucken“ (einziger Eintrag), die Zugriffstaste des Knopfes
# „Drucken“/„Print“ druckt
schritt drucken-wahl "key Tab" "wait 300" "key Down"
schritt drucken-fertig "chord alt $DRUCK" "wait 4000"
DRUCK=$(find "$QD_PROFILE/home" -name '*.pdf' -newer "$L/vor-druck" -not -path '*/pdf/*' | head -1)
if [ -n "$DRUCK" ]; then
    echo "druck: $DRUCK $(python3 -I "$W/pdfseiten.py" "$DRUCK") Seiten" >>"$L/ablauf.log"
    cp "$DRUCK" "$OUT/druck-$SPR-$RES.pdf"
else
    echo "druck: keine Datei" >>"$L/ablauf.log"
    schritt drucken-abbruch "key Escape"
fi
# Fehlermeldung: zweiter Aufruf mit kaputter Datei → neues Fenster mit Meldung
"$BIN" "$T/kaputt.pdf" >>"$L/app2.log" 2>&1
schritt fehler "wait 1500"
schritt fehler-zu "key Escape" "wait 500" "chord ctrl w"
# Passwortabfrage: falsch, dann richtig
"$BIN" "$T/geschuetzt.pdf" >>"$L/app2.log" 2>&1
schritt passwort "wait 1500"
schritt passwort-falsch "type falsch" "key Return" "wait 800"
schritt passwort-richtig "type querencia" "key Return" "wait 1500"
schritt passwort-zu "chord ctrl w"
# Ende: Strg+Q beendet das Programm
q "chord ctrl q" "wait 2500"
if kill -0 "$PID" 2>/dev/null; then
    echo "ENDE Strg+Q: läuft noch" >"$L/status.txt"
else
    wait "$PID"; echo "ENDE Strg+Q: beendet, Exitcode $?" >"$L/status.txt"
fi
echo "katalog: ${MO:-keiner}" >>"$L/status.txt"
cat "$L/status.txt" >>"$L/ablauf.log"
touch "$L/FERTIG"
