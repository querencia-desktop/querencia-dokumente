# Querencia Dokumente

Klassischer PDF-Betrachter für alle, die Dokumente lesen, durchsuchen und
drucken wollen. Teil von
[Querencia Desktop](https://github.com/querencia-desktop/querencia) – einer
klassischen Desktopumgebung im GNOME-2-Stil für Wayland (labwc, Rust, GTK 4).

*English summary:* A classic PDF document viewer modelled on Evince 2.30.
Part of Querencia Desktop, a classic GNOME-2-style desktop for Wayland.
Alpha software; documentation is in German.

> **Alpha.** Nur PDF; keine Links, kein Inhaltsverzeichnis, keine
> Anmerkungen, keine Formulare. Keine Sicherheits- oder Datenverlustgarantie.

## Zweck

- PDF-Dateien öffnen (Dialog, Kommandozeile, Ziehen und Ablegen), auch
  passwortgeschützte (Passwortabfrage über Poppler)
- fortlaufend blättern, Seitenleiste mit Miniaturen, „Seite X von Y“ mit
  Eingabefeld (auch Seitenbezeichnungen wie „iv“)
- Zoom in Stufen (10 % bis 400 %), „Ganze Seite“, „Seitenbreite“,
  Strg+Mausrad
- Textsuche mit Hervorhebung aller Treffer und Weiter/Zurück, Groß- und
  Kleinschreibung wahlweise
- markierten Text kopieren (Maus, Strg+A, Strg+C; mittlere Maustaste)
- Eigenschaften (Titel, Autor, Thema, Seiten, Papiergröße, Daten …)
- Drucken über den GTK-Druckdialog, Vollbild, zuletzt geöffnete Dokumente
- Bewusst **nicht** Teil dieses Programms: Bearbeiten, Speichern,
  Anmerkungen, Formulare, Signaturen, andere Formate (PostScript, DjVu, XPS,
  Comics) – Querencia Dokumente **schreibt nie** in Dokumente.

Anwendungskennung (App-ID): `org.querencia.Dokumente` · Binary:
`querencia-dokumente` · Debian-Paket: `querencia-dokumente`

| Taste | Wirkung |
|---|---|
| Strg+O, Strg+P, Alt+Eingabe | Öffnen, Drucken, Eigenschaften |
| Strg+W, Strg+Q | Fenster schließen, Programm beenden |
| Strg+F, Strg+G / F3, Umschalt+Strg+G / Umschalt+F3, Escape | Suchen, weiter, zurück, Suchleiste schließen |
| Strg+C, Strg+A | Kopieren, alles auswählen (in der Dokumentansicht) |
| Strg+Plus/Strg+=, Strg+Minus, Strg+0, Strg+Mausrad, +/− | Zoom, 100 % |
| Bild↓/Bild↑, Pos1/Ende, Strg+Bild↓/↑, Strg+Pos1/Ende | nächste/vorige, erste/letzte Seite |
| Pfeiltasten, Leertaste/Umschalt+Leertaste | Bildlauf |
| Strg+L | Seitenfeld |
| F9, F11 | Seitenleiste, Vollbild (Escape beendet) |

## Stand

| | |
|---|---|
| Version | 0.1.3 (Tag `dokumente/0.1.3`; keine weißen Seiten mehr, wenn das offene PDF neu geschrieben wird); seit 0.1.2 dunkles Thema auch beim Start |
| Letzte Prüfung | 21.10.2026: 52 Unit-Tests; Probe mit Überschreiben und Löschen des offenen PDF im Betrieb, hell und dunkel gegen 0.1.2 (isoliert) |
| Bekannte Lücken | siehe [docs/testing.md](docs/testing.md), Abschnitt „Nicht getestet“ |
| Getestet auf | Ubuntu 26.04, labwc 0.9.8, GTK 4.22, Poppler 26.01, Rust 1.93 |
| Nicht getestet | andere Distributionen, HiDPI, Mehrmonitor, echte Drucker |

Änderungen: [CHANGELOG.md](CHANGELOG.md).

## Bauen und Prüfen

`cargo build --release --locked`, danach `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` und `python3 werkzeug/i18n.py pruefen` (laufen ohne Bildschirm; die ersten drei sind die CI `.github/workflows/ci.yml`).
Voraussetzungen, Pfade, Übersetzungen, Paket: [docs/building.md](docs/building.md); isolierte GUI-Prüfung (`bash werkzeug/lauf.sh de 1024x768`, nie in der laufenden Sitzung) und Messung: [docs/testing.md](docs/testing.md); Integration: [docs/integration.md](docs/integration.md).

## Vorbild und Abgrenzung

Vorbild für Bedienung und Aussehen ist **Evince 2.30 aus GNOME 2**: Menüs
Datei/Bearbeiten/Ansicht/Gehe zu/Hilfe, Werkzeugleiste mit Seitenfeld und
Zoom, Seitenleiste mit Miniaturen, Suchleiste unten. Querencia übernimmt
davon die Bedienideen, **keinen Quellcode, keine Texte, keine Grafiken**.
Das Projekt ist mit Evince und dessen Autoren weder verbunden noch von ihnen
unterstützt.

Abweichungen vom Vorbild und Dinge, die bewusst fehlen:

- nur PDF; keine Links, kein Inhaltsverzeichnis, keine Doppelseiten, kein
  Präsentationsmodus, kein Drehen, kein Neu laden bei Dateiänderung
- eine Statusleiste (Evince 2.30 hat keine), wie die anderen Querencia-Programme
- „Seitenbreite“ und „Ganze Seite“ gelten für die breiteste bzw. größte
  Seite; alle Seiten haben denselben Maßstab
- Zoom endet bei 400 % (größere Seiten würden zu viel Speicher brauchen)

Gerendert wird in einem Hintergrundfaden mit eigenem Poppler-Dokument; ein
Seiten-Cache mit Obergrenze (128 MB, `QUERENCIA_DOKUMENTE_CACHE_MB`) und ein
Miniaturen-Cache (32 MB) halten den Speicher begrenzt; die Suche läuft in
einem zweiten Faden.

## Lizenz

- Programm: GPL-3.0-or-later, siehe [LICENSE](LICENSE). Copyright 2026
  Querencia Desktop contributors; Urheberhinweis in
  [COPYRIGHT](COPYRIGHT).
- **Poppler:** Querencia Dokumente linkt dynamisch gegen `libpoppler-glib`
  (Poppler, GPL-2.0 oder GPL-3.0; Ubuntu-Paket `libpoppler-glib8t64`). Das
  gelinkte Programm wird deshalb nach den Bedingungen der GPL verbreitet;
  GPL-3.0-or-later ist damit verträglich. Weitere Bibliotheken: GTK 4,
  GLib, Cairo, Pango (LGPL), die Rust-Bindings gtk-rs und poppler-rs (MIT).
- `werkzeug/virtual-keyboard-unstable-v1.xml` (nur Testwerkzeug) behält den
  MIT-Vermerk im Dateikopf.
- Übersetzungen (`po/`) stehen unter derselben Lizenz wie das Programm.

Name und Logo „Querencia“ sind keine freien Inhalte.

## Fehler melden

Fehler, Wünsche und Fragen zu Querencia gehen an die gemeinsame Meldestelle:
<https://github.com/querencia-desktop/querencia/issues/new/choose>.
Sicherheitslücken bitte nicht öffentlich melden, siehe [SECURITY.md](SECURITY.md).
