# Integration in die laufende Sitzung

Vorlage und Skript liegen unter `integration/` (wird beim Veröffentlichen aus
der Geschichte gefiltert, ENTSCHEIDUNGEN Nr. 13).

| Datei | Zweck |
|---|---|
| `integration/werkzeug/dokumente-010-integrieren.sh` | Skript (`pruefen`, `aktivieren`, `zurueck`), aus der `.sh.in` mit eingetragenen Prüfsummen |
| `integration/werkzeug/vorlage-010-erzeugen.sh <commit>` | baut `integration/vorlage/` (Release-Binary, `de.mo`, Symbol, Starter-Vorlage, `ablage.sha256`, `COMMIT`, `SHA256SUMS`) und erzeugt das Skript |
| `integration/werkzeug/probe-010.sh <ordner>` | Probe mit umgelenktem `HOME` |
| `integration/vorlage/` | Vorlage, schreibgeschützt (0444/0555); Binary, Katalog und Symbol nicht im Git |

## Was `aktivieren` tut

Alle Pfade folgen aus `$HOME`. Ist `HOME` das echte Heimatverzeichnis, ist
der Lauf live und braucht `DOK_FREIGABE=ja` (nur Status-Session nach Prüfung).

1. Ablage `~/opt/querencia/dokumente/<sha16>/` (Verzeichnisse 0555): Binary
   0555, `share/locale/de/LC_MESSAGES/querencia-dokumente.mo` und
   `SHA256SUMS` 0444, daneben `.herkunft`. Sie entsteht unter einem
   Hilfsnamen, wird geprüft und erst dann umbenannt.
2. Link `~/opt/querencia/bin/querencia-dokumente` auf die Ablage.
3. Symbol nach `~/.local/share/icons/hicolor/{16,22,24,32,64,128,256}x…/apps/`
   und `scalable/apps/` (kein 48×48), danach `touch` auf den hicolor-Ordner.
4. Starter `~/.local/share/applications/org.querencia.Dokumente.desktop`
   (Exec/TryExec auf die Ablage, „Dokumentenbetrachter“ unter Büro,
   `MimeType=application/pdf;`), danach `update-desktop-database`.
5. In `~/.config/mimeapps.list` unter `[Default Applications]` die Zeile
   `application/pdf=org.querencia.Dokumente.desktop`; der bisherige Wert
   (oder „keiner“) wird in `~/.local/state/querencia/dokumente-010-mime-vorher.txt`
   gemerkt. Alle anderen Zeilen bleiben.
6. Schlusskontrolle (Zustände, `gio mime application/pdf`, Invarianten),
   Journal `~/opt/querencia/protokoll.txt` mit Rückfallbefehl.

Vorher sichert das Skript Link, Starter, `mimeinfo.cache`, `mimeapps.list`
und Symbole nach `~/.local/state/querencia/sicherung-dokumente-010-*`. Bei
einem Fehler oder Signal rollt es zurück (Exit 1). Es startet nichts.

## Was `zurueck` tut

In umgekehrter Reihenfolge: PDF-Standard auf den gemerkten Wert (nur wenn er
noch auf Dokumente steht), Starter entfernen und `mimeinfo.cache` erneuern,
Symbol entfernen, Link entfernen. Die Ablage bleibt liegen (unveränderliche
Ablagen werden nie gelöscht).

## Befehle

```bash
bash integration/werkzeug/dokumente-010-integrieren.sh pruefen
DOK_FREIGABE=ja bash integration/werkzeug/dokumente-010-integrieren.sh aktivieren
DOK_FREIGABE=ja bash integration/werkzeug/dokumente-010-integrieren.sh zurueck
bash integration/werkzeug/probe-010.sh "$PWD/gui-test/probe"
```

Exit-Codes: 0 erledigt, 1 zurückgerollt, 2 verweigert (nichts geändert),
3 Rückrollen unvollständig (Sicherung im State-Ordner).
