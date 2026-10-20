# Änderungsprotokoll

Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionen
nach [Semantic Versioning](https://semver.org/lang/de/). Solange die Hauptversion
0 ist, kann sich alles ändern. Jede Zeile nennt, was Nutzer merken; Unfertiges
steht als „unfertig“ da.

## [0.1.2] – 2026-10-21

### Behoben
- Unter „Dunkle Darstellung bevorzugen“ startet Dokumente dunkel, sobald
  das Thema eine dunkle Fassung hat (Querencia-Klassisch ab 0.2, QD-081):
  GTK 4.22 lädt das dunkle Thema-CSS sonst nur bei einem Wechsel im
  laufenden Programm. Dokumente meldet `gtk-interface-color-scheme` einmal
  beim Start. Hell ändert sich nichts.

### Geändert
- Seiten und Miniaturen bleiben auch dunkel weiß wie Papier; die Fläche
  hinter den Seiten bleibt grau. Die Programm-CSS enthält keine Farbe, die
  dem Thema widerspricht.
- Texte und Katalog unverändert.

## [0.1.1] – 2026-10-20

### Behoben
- Die Fenster „Fehler“, „Eigenschaften“ (Alt+Eingabe), „Info“ und die
  Passwortabfrage haben bei jedem Öffnen eine Titelleiste, nicht nur beim
  ersten (QD-076). Sie werden beim Schließen zerstört und beim Öffnen neu
  gebaut, statt verborgen und wieder gezeigt zu werden: labwc 0.9.8
  verknüpft die Dekorationsanfrage nur mit dem ersten xdg_toplevel einer
  Fläche, GTK 4.22 legt beim Wiederzeigen ein neues an. Muster Querencia
  Bilder 0.2.3. Ein offenes Fenster wird weiter wiederverwendet.
- Nach einem falschen Passwort erscheint die Abfrage als neues Fenster mit
  dem Hinweis; das richtige Passwort entsperrt das Dokument wie bisher.

### Geändert
- Die Nebenfenster gehören zur Anwendung (App-ID `org.querencia.Dokumente`
  statt des Programmnamens, Symbol in der Fensterliste wie das
  Hauptfenster), werden mit ihrem Hauptfenster geschlossen und schließen
  sich im nächsten Leerlauf nach Escape oder einem Knopf.
- Texte und Katalog unverändert.

## [0.1.0] – 2026-10-20

Erste interne Alpha (QD-019). Liest PDF-Dateien wie Evince 2.30; bearbeitet
nichts.

### Hinzugefügt
- PDF öffnen über den Dialog, die Kommandozeile und Ziehen und Ablegen; je
  Dokument ein Fenster, ein leeres Fenster wird wiederverwendet.
- Passwortabfrage für verschlüsselte PDFs (falsches Passwort: Hinweis und
  neuer Versuch); Fehlermeldung bei beschädigten oder unlesbaren Dateien.
- Fortlaufende Ansicht mit Bildlauf, Seitenleiste mit Miniaturen (F9),
  Seitenfeld „X von Y“ (auch Seitenbezeichnungen), Blättern mit Bild↑/↓,
  Pos1/Ende und der Werkzeugleiste.
- Zoom in Stufen bis 400 %, „Ganze Seite“, „Seitenbreite“, Strg+Mausrad um
  den Mauszeiger, Zoom-Menü in der Werkzeugleiste.
- Textsuche (Strg+F) mit Hervorhebung aller Treffer, Weiter/Zurück,
  Trefferzähler, Groß-/Kleinschreibung; läuft im Hintergrund.
- Text mit der Maus markieren, alles auswählen, kopieren (auch für die
  mittlere Maustaste).
- Eigenschaften (Titel, Ort, Thema, Autor, Stichwörter, Ersteller, Erzeuger,
  Daten, Seitenzahl, Format, Sicherheit, Papiergröße, Dateigröße).
- Drucken über den GTK-Druckdialog; Papier folgt dem Format des Dokuments;
  Querseiten werden gedreht eingepasst.
- Vollbild (F11), Werkzeug-, Status- und Seitenleiste ein-/ausblendbar,
  zuletzt geöffnete Dokumente im Menü „Datei“.
- Rendern in einem Hintergrundfaden mit Seiten-Cache (Obergrenze 128 MB)
  und Miniaturen-Cache (32 MB).
- Oberfläche englisch und deutsch über GNU gettext; eigenes Programmsymbol.

### Bekannte Einschränkungen
- Keine Links, kein Inhaltsverzeichnis, keine Anmerkungen oder Formulare,
  kein Neu laden bei Dateiänderung, kein Drehen.
- Nur PDF.

[0.1.0]: https://github.com/querencia-desktop/querencia-dokumente/releases/tag/dokumente/0.1.0
