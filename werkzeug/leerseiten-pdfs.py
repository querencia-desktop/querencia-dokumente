#!/usr/bin/env python3
"""Dokumente 0.1.3: Test-PDFs für die Leerseiten-Prüfung (testdaten/).

Läuft mit dem Python, in dem WeasyPrint 70 steckt (wie das Befund-PDF):
  ~/.cache/querencia-pdf-venv/bin/python -I werkzeug/leerseiten-pdfs.py \\
      testdaten ~/querencia-web/fonts
Erzeugt (nur selbst erzeugter Blindtext, keine Namen, keine Netzdaten):
  leer-woff2.pdf          1 Seite A4, eingebettete variable WOFF2-Schriften
                          (Inter, JetBrains Mono; SIL OFL 1.1)
  leer-standard.pdf       1 Seite A4, Standardschrift Helvetica (nicht
                          eingebettet, ohne WeasyPrint geschrieben)
  leer-bild.pdf           1 Seite A4 mit einem Rasterbild (PNG, selbst erzeugt)
  leer-30.pdf             30 Seiten A4 wie das Befund-PDF (Überschriften,
                          Codeblöcke mit Hintergrund, Linien, WOFF2)
  leer-30-neu.pdf         dieselben 30 Seiten mit geändertem Text auf Seite 3
                          (zweite Fassung: Datei wird beim Lesen überschrieben)
"""

import os
import struct
import sys
import zlib

ZIEL = sys.argv[1]
SCHRIFTEN = os.path.abspath(os.path.expanduser(sys.argv[2]))

WORTE = (
    "Dies ist ein Testdokument für Querencia Dokumente mit selbst erzeugtem "
    "Blindtext und Umlauten wie ä ö ü ß. Jede Zeile prüft Schrift, Farbe "
    "und Abstand der Hauptansicht gegen die Miniatur der Seitenleiste."
).split()

CSS = f"""
@font-face {{ font-family: Inter; src: url("file://{SCHRIFTEN}/inter-variable.woff2") format("woff2");
  font-weight: 100 900; }}
@font-face {{ font-family: Mono; src: url("file://{SCHRIFTEN}/jetbrains-mono-variable.woff2") format("woff2");
  font-weight: 100 800; }}
@page {{ size: A4; margin: 22mm 20mm; @bottom-center {{ content: "Testseite " counter(page);
  font: 8pt Inter; color: #888; }} }}
body {{ font: 10.5pt/1.45 Inter; color: #222; }}
h1 {{ font-weight: 800; font-size: 22pt; margin: 0 0 8pt; }}
h2 {{ font-weight: 650; font-size: 14pt; border-bottom: 1.5pt solid #e07a3a; padding-bottom: 2pt; }}
.teil {{ color: #e07a3a; font-size: 8pt; letter-spacing: 1pt; font-weight: 500; }}
pre {{ font: 8.5pt/1.4 Mono; background: #f3f1ee; padding: 6pt 8pt; white-space: pre-wrap; }}
section {{ break-before: page; }}
"""


def absatz(n, versatz=0):
    return " ".join(WORTE[(i + versatz) % len(WORTE)] for i in range(n)) + "."


def seite(nr, neu=False):
    extra = " Zweite Fassung: dieser Absatz ist länger." * 6 if neu and nr == 3 else ""
    teile = [f'<p class="teil">TEIL {nr}</p><h1>Kapitel {nr}: Seitenprobe</h1>',
             f"<p>{absatz(60, nr)}{extra}</p>",
             f"<h2>{nr}.1 Abschnitt mit Code</h2>",
             f"<pre>Zeile {nr}: echo probe | wc -c   # {absatz(12, nr)}\n"
             + "".join(f"schritt {k}: {absatz(8, k)}\n" for k in range(6)) + "</pre>",
             f"<p>{absatz(80, nr + 3)}</p>",
             f"<h2>{nr}.2 Liste</h2><ul>"
             + "".join(f"<li><b>Punkt {k}</b> {absatz(14, k)}</li>" for k in range(6)) + "</ul>",
             f"<p>{absatz(50, nr + 7)}</p>"]
    return "<section>" + "".join(teile) + "</section>"


def html(body):
    return f"<!doctype html><html lang=de><head><meta charset=utf-8><style>{CSS}</style></head><body>{body}</body></html>"


def png(breite, hoehe):
    """Farbverlauf mit Kreis, RGB, ohne Fremdbibliothek."""
    zeilen = b""
    for y in range(hoehe):
        zeilen += b"\0"
        for x in range(breite):
            innen = (x - breite / 2) ** 2 + (y - hoehe / 2) ** 2 < (hoehe / 3) ** 2
            r, g, b = (224, 122, 58) if innen else (40 + 180 * x // breite, 90, 60 + 160 * y // hoehe)
            zeilen += bytes((r, g, b))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", breite, hoehe, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(zeilen, 9)) + chunk(b"IEND", b""))


def standard_pdf(pfad):
    """Eine Seite A4, Helvetica (Standard-14, nicht eingebettet)."""
    zeilen = ["BT /F1 22 Tf 57 770 Td (Standardschrift Helvetica) Tj ET"]
    y = 730
    for k in range(40):
        text = absatz(12, k).replace("ä", "ae").replace("ö", "oe").replace("ü", "ue").replace("ß", "ss")
        zeilen.append(f"BT /F1 10 Tf 57 {y} Td ({text}) Tj ET")
        y -= 16
    zeilen.append("0.88 0.48 0.23 RG 2 w 57 750 m 538 750 l S")
    inhalt = "\n".join(zeilen).encode("latin-1")
    objekte = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(inhalt) + inhalt + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    ]
    out = b"%PDF-1.4\n"
    stellen = []
    for i, o in enumerate(objekte, 1):
        stellen.append(len(out))
        out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objekte) + 1)
    out += b"".join(b"%010d 00000 n \n" % s for s in stellen)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objekte) + 1, xref)
    with open(pfad, "wb") as f:
        f.write(out)


def main():
    from weasyprint import HTML

    os.makedirs(ZIEL, exist_ok=True)
    ziel = os.path.abspath(ZIEL)

    def schreiben(name, body):
        HTML(string=html(body), base_url=ziel).write_pdf(os.path.join(ziel, name))

    schreiben("leer-woff2.pdf", seite(1))
    bild = os.path.join(ziel, "leer-bild.png")
    with open(bild, "wb") as f:
        f.write(png(320, 200))
    schreiben("leer-bild.pdf", '<section><h1>Seite mit Bild</h1>'
              f'<p>{absatz(30)}</p><img src="file://{bild}" style="width:140mm"></section>')
    os.remove(bild)
    standard_pdf(os.path.join(ziel, "leer-standard.pdf"))
    schreiben("leer-30.pdf", "".join(seite(n) for n in range(1, 31)))
    schreiben("leer-30-neu.pdf", "".join(seite(n, neu=True) for n in range(1, 31)))


if __name__ == "__main__":
    main()
