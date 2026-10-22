# Bauen

## Voraussetzungen

Ubuntu 26.04 mit `build-essential pkg-config rustc cargo libgtk-4-dev
libpoppler-glib-dev gettext` (Rust 1.93 aus dem Archiv reicht). Crates:
gtk4 0.11, poppler-rs 0.26 (Feature `v20_9`), cairo-rs 0.22, gettext-rs 0.8
(`gettext-system`), futures-channel/-util 0.3; alle Versionen in
`Cargo.lock`. Die Bauskripte arbeiten offline (`--offline --locked`).

## Release mit Prüfungen

```bash
bash werkzeug/bauen.sh            # fmt, Clippy, Tests, dann Release
bash werkzeug/bauen.sh pruefen    # nur fmt, Clippy, Tests
bash werkzeug/bauen.sh release    # nur Release
```

- Alles läuft auf einem der zwei gemeinsamen Bau-Plätze
  (`~/.local/state/querencia/bau-platz.sh`, setzt `nice -n 10` und
  `CARGO_BUILD_JOBS=4`) mit eigenem `TMPDIR` (`tmp/`). GUI- und Messläufe
  laufen nie unter einem Bau-Platz.
- Der Release-Build läuft mit
  `--remap-path-prefix=<repo>=/usr/src/querencia-dokumente` und
  `--remap-path-prefix=<CARGO_HOME>=/usr/share/cargo` (wie
  `querencia-packaging`) nach `target/remap/release/`. Das Skript zählt am
  Ende die Home-Pfade im Binary (`strings -a … | grep -c $HOME`, Soll 0).
  Ohne Remap enthält ein Release-Binary Panik-Orte aus dem Cargo-Cache
  (27 Treffer beim Rechner).
- Protokolle: `nachweis/bau/<art>.log`, Prüfsumme `nachweis/bau/release-sha256.txt`.

## Übersetzungen

Englisch steht im Code (msgid), Deutsch in `po/de.po`. Werkzeug ist
`werkzeug/i18n.py` aus dem Editor (nur Python-Standardbibliothek):

```bash
python3 werkzeug/i18n.py pot      # po/querencia-dokumente.pot aus src/ und data/*.desktop.in
python3 werkzeug/i18n.py update   # po/de.po nachziehen, dann übersetzen
python3 werkzeug/i18n.py desktop data/org.querencia.Dokumente.desktop.in data/org.querencia.Dokumente.desktop
python3 werkzeug/i18n.py pruefen  # muss PASS melden
python3 werkzeug/i18n.py mo <ziel>
msgfmt -c -o /dev/null po/de.po   # Gegenprobe mit GNU gettext
```

Die Binary sucht den Katalog `querencia-dokumente.mo` in dieser Reihenfolge:
`QUERENCIA_DOKUMENTE_LOCALEDIR`, `<Ordner der Binary>/share/locale`,
`<Präfix>/share/locale`, `/usr/share/locale`. Ohne Katalog ist die
Oberfläche englisch.

## Symbol und Ressource

Quelle ist `data/icons/hicolor/scalable/apps/org.querencia.Dokumente.svg`
(Entwurf im Stil der Querencia-Symbole: graue Linien #5a5a5a, Akzent
#F07746; das Symbole-Repo übernimmt ihn später). Nach einer Änderung:

```bash
python3 -I werkzeug/symbol-rendern.py     # PNG 16–256, nie 48x48
glib-compile-resources --sourcedir=data/icons/hicolor/scalable/apps \
  --target=data/resource/dokumente.gresource data/resource/dokumente.gresource.xml
```

Der Unit-Test `resource::tests` schlägt fehl, solange die Ressource nicht
zum SVG passt. Das Info-Fenster nimmt das Symbol aus der Ressource unter
eigenem Namen (`org.querencia.Dokumente-info`), damit es nie neben dem
PNG-Satz der App-ID im Symbolthema liegt (GTK-4.22-Symbolabsturz).

## Paket (später)

- `/usr/bin/querencia-dokumente`
- `/usr/share/locale/de/LC_MESSAGES/querencia-dokumente.mo` (aus `po/de.po`
  mit `msgfmt`; Build-Abhängigkeit `gettext`)
- `/usr/share/applications/org.querencia.Dokumente.desktop`
  (`data/org.querencia.Dokumente.desktop`)
- `/usr/share/icons/hicolor/{16…256,scalable}/apps/org.querencia.Dokumente.*`
- `/usr/share/metainfo/org.querencia.Dokumente.metainfo.xml`
- Abhängigkeit `libpoppler-glib8t64`; `debian/copyright` mit dem Hinweis
  „gelinkt gegen Poppler (GPL-2|3)“ (siehe `COPYRIGHT`).
