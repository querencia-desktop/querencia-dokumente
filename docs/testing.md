# Testen

## Ohne Bildschirm

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
python3 werkzeug/i18n.py pruefen
```

Die Unit-Tests decken ab:

- **Zoomlogik** (`zoom.rs`): Stufen, Grenzen, Seitenbreite, Ganze Seite,
  Prozentangabe.
- **Seitenmodell** (`layout.rs`): Stapeln mit Rand und Abstand, Zentrieren,
  sichtbare Seiten, aktuelle Seite (auch 500 Seiten bei 10 %), Anker beim
  Zoomen, Trefferpunkte in PDF-Punkt.
- **Suche** (`search.rs`): Umrechnung der Poppler-Rechtecke (Ursprung unten
  links), Ordnen, Weiter/Zurück ringsum, Zählen; dazu im Hintergrundfaden
  (`render.rs`) die Suche über ein erzeugtes PDF mit und ohne Groß- und
  Kleinschreibung.
- **Auswahl** (`selection.rs`, `document.rs`): Bereiche über Seitengrenzen,
  ausgewählter Text und Rechtecke aus Poppler.
- **Cache** (`cache.rs`): Obergrenze, älteste zuerst, geschützte sichtbare
  Seiten, Ersatzbild in anderem Maßstab.
- **Öffnen** (`document.rs`): Metadaten, beschädigt, fehlend, Passwort
  (fehlt, falsch, richtig) mit `testdaten/geschuetzt.pdf`.
- Rendern im Hintergrundfaden (Bildgröße, Grenzen), Druck-Einpassung,
  Papierformate, Katalog vollständig, Ressource aktuell.

Die Tests erzeugen ihre PDFs mit Cairo selbst; `testdaten/geschuetzt.pdf`
stammt aus `werkzeug/testpdf.py` (RC4-40, Passwort `querencia`).

## GUI, isoliert

Nie in der laufenden Sitzung. `werkzeug/isoliert.py` startet einen eigenen
labwc 0.9.8 ohne Bildschirm (`WLR_BACKENDS=headless`) mit eigenem D-Bus,
eigenem `XDG_RUNTIME_DIR` (`laufzeit/qd-runtime-*`), eigenem Profil und
**umgelenktem `HOME`** (`gui-test/<name>/home`). Portale, gvfs und CUPS sind
im Wegwerfprofil abgeschaltet; `gtk-print-backends=file` lässt im
Druckdialog nur „In Datei drucken“ übrig. Eingaben kommen über
`werkzeug/bin/qeingabe` (virtuelle Tastatur und Zeiger, verweigert sich
außerhalb von `qd-runtime-*`), Bilder über `grim`.

```bash
bash werkzeug/werkzeug-bauen.sh                 # einmalig: qeingabe, qausgabe
bash werkzeug/lauf.sh de 1024x768               # Ablage-Quelle: integration/vorlage
bash werkzeug/lauf.sh en 1920x1080
SZENARIO=messung.sh bash werkzeug/lauf.sh de 1920x1080 messung
bash werkzeug/quelle.sh target/remap/release/querencia-dokumente "$PWD/gui-test/quelle"   # vor der Vorlage
```

`de` = wie die Sitzung (`LANG=en_US.UTF-8`, `LANGUAGE=de`), `en` =
`LANGUAGE=en`. Gestartet wird ein Link auf eine Ablage-Kopie mit
`share/locale`; im de-Lauf muss `/proc/<pid>/maps` den Katalog nennen.

`werkzeug/szenario.sh` (49 Schritte): fünf Menüs und „Zuletzt geöffnet“,
Blättern (Bild↓, Ende, Pos1, Pfeile), Seitenfeld (Strg+L, Seite 5 quer),
Zoom (Strg+0, Strg+Plus, Strg+Minus, Ganze Seite, Strg+Mausrad,
Seitenbreite), Suche (Treffer, weiter, zurück, viele Treffer, F3), Auswahl
mit der Maus und Strg+A mit `wl-paste`-Prüfung, Eigenschaften, Seitenleiste,
Vollbild, Info, Öffnen-Dialog, Drucken in eine PDF-Datei (Seitenzahl über
Poppler geprüft), Fehlermeldung (`kaputt.pdf`), Passwort falsch und richtig,
Strg+W, Strg+Q mit Exitcode.

`werkzeug/messung.sh`: 500-Seiten-PDF (`gross-500.pdf`), Zeit bis zur ersten
Seite, Speicher (VmRSS/VmHWM) nach Sprüngen, 80 × Bild↓, 400 %, kleinster
Stufe, Suche über alle Seiten mit Blättern währenddessen.

Test-PDFs: `werkzeug/testpdf.py <ordner>` (nur selbst erzeugter Blindtext).

Nebenfenster (QD-076, ab 0.1.1): Szenario `sicht-szenario.sh` im
Arbeitsordner `~/projects/querencia-dokumente-011/werkzeug/` (außerhalb des
Repos, wie die Integration). Fehler, Eigenschaften, Info und Passwort je
dreimal öffnen und schließen; die Titelleiste wird über die Pixel ihrer
Farbe gezählt (`titelleiste.py`), GTK-Meldungen im Programmprotokoll.

Belege: `nachweis/gui/<name>/` (Bilder, `ablauf.log`, `app.log`,
`status.txt`, Zwischenablage, Druck-PDF, `messung.txt`).

## Nicht getestet

- Anblick in der echten Sitzung, Panel-Menüeintrag, Doppelklick in Files
- echte Drucker (nur „In Datei drucken“)
- Ziehen und Ablegen aus einem anderen Programm
- HiDPI (Skalierung > 1), Mehrmonitor
- sehr große Einzelseiten (Poster) jenseits der Bildgrenzen-Unit-Tests
- PDFs mit eingebetteten Schriften anderer Erzeuger, Formulare, Anmerkungen
- `LANG=C`: glibc beachtet dann `LANGUAGE` nicht, die Oberfläche wäre englisch
