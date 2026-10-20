#!/usr/bin/env python3
"""Test-PDFs für Querencia Dokumente erzeugen (nur Standardbibliothek).

Alle Inhalte sind selbst erzeugt (Blindtext, Seitennummern, Formen); kein
Fremddokument. Schriften: die Standard-14-Schrift Helvetica (nicht
eingebettet, Poppler ersetzt sie über fontconfig), WinAnsiEncoding für
Umlaute.

Aufruf: testpdf.py <zielordner>
Erzeugt:
  bericht.pdf      12 Seiten A4, eine davon quer, Titel/Autor/Thema gesetzt,
                   Suchwort „Querencia“ auf mehreren Seiten
  brief.pdf        1 Seite US Letter ohne Metadaten
  gross-500.pdf    500 Seiten A4 mit Text und Vektorgrafik (Messungen)
  geschuetzt.pdf   3 Seiten, RC4-40 verschlüsselt, Passwort „querencia“
  kaputt.pdf       abgeschnittene Datei (Fehlermeldung)
"""

import hashlib
import os
import sys

PAD = bytes.fromhex(
    "28BF4E5E4E758A4164004E56FFFA01082E2E00B6D0683E802F0CA9FE6453697A"
)

LOREM = (
    "Dies ist ein Testdokument für Querencia Dokumente. Es enthält nur selbst "
    "erzeugten Blindtext mit Umlauten wie ä, ö, ü und ß, damit Suche, Auswahl "
    "und Kopieren geprüft werden können. Die Zeilen sind absichtlich gleich "
    "lang und der Inhalt wiederholt sich auf jeder Seite."
).split()


def rc4(key: bytes, data: bytes) -> bytes:
    s = list(range(256))
    j = 0
    for i in range(256):
        j = (j + s[i] + key[i % len(key)]) % 256
        s[i], s[j] = s[j], s[i]
    out = bytearray()
    i = j = 0
    for b in data:
        i = (i + 1) % 256
        j = (j + s[i]) % 256
        s[i], s[j] = s[j], s[i]
        out.append(b ^ s[(s[i] + s[j]) % 256])
    return bytes(out)


def pdf_string(text: str) -> bytes:
    raw = text.encode("cp1252")
    return b"(" + raw.replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)") + b")"


def lines_of(words, width):
    line, out = [], []
    for w in words:
        if sum(len(x) + 1 for x in line) + len(w) > width:
            out.append(" ".join(line))
            line = []
        line.append(w)
    if line:
        out.append(" ".join(line))
    return out


def page_content(n: int, total: int, w: float, h: float, heading: str, extra: str, shapes: bool) -> bytes:
    ops = []
    if shapes:
        # Vektorgrafik: Kopfbalken, Kreis aus Bézierkurven, Raster
        ops.append("0.94 0.47 0.27 rg 50 %.1f %.1f 6 re f" % (h - 70, w - 100))
        ops.append("0.6 g 0.5 w")
        for k in range(12):
            y = 120 + k * 10
            ops.append("%.1f %d m %.1f %d l S" % (w - 230, y, w - 60, y))
        cx, cy, r = w - 145, 260.0, 60.0
        c = 0.5523 * r
        ops.append("0.35 0.35 0.35 RG 2 w")
        ops.append(
            "%.1f %.1f m %.1f %.1f %.1f %.1f %.1f %.1f c %.1f %.1f %.1f %.1f %.1f %.1f c "
            "%.1f %.1f %.1f %.1f %.1f %.1f c %.1f %.1f %.1f %.1f %.1f %.1f c S"
            % (cx + r, cy, cx + r, cy + c, cx + c, cy + r, cx, cy + r,
               cx - c, cy + r, cx - r, cy + c, cx - r, cy,
               cx - r, cy - c, cx - c, cy - r, cx, cy - r,
               cx + c, cy - r, cx + r, cy - c, cx + r, cy)
        )
    ops.append("0 g BT /F1 20 Tf 50 %.1f Td %s Tj ET" % (h - 110, pdf_string(heading).decode("latin-1")))
    y = h - 150
    body = lines_of(LOREM * 3 + extra.split(), 80 if w > h else 62)
    for line in body:
        if y < 120:
            break
        ops.append("BT /F1 11 Tf 50 %.1f Td %s Tj ET" % (y, pdf_string(line).decode("latin-1")))
        y -= 15
    ops.append("BT /F1 10 Tf %.1f 40 Td %s Tj ET" % (w / 2 - 30, pdf_string(f"Seite {n} von {total}").decode("latin-1")))
    return "\n".join(ops).encode("latin-1")


class Writer:
    def __init__(self):
        self.objects = []  # (nummer, bytes ohne „n 0 obj“)

    def add(self, body: bytes) -> int:
        self.objects.append(body)
        return len(self.objects)

    def set(self, num: int, body: bytes):
        self.objects[num - 1] = body

    def build(self, root: int, info: int | None, encrypt=None) -> bytes:
        doc_id = hashlib.md5(b"querencia-dokumente-test" + str(len(self.objects)).encode()).digest()
        key = None
        enc_num = None
        if encrypt is not None:
            key, enc_body = encrypt(doc_id)
            enc_num = self.add(enc_body)
        out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
        offsets = []
        for i, body in enumerate(self.objects, start=1):
            if key is not None and i != enc_num:
                body = self._encrypt_obj(body, i, key)
            offsets.append(len(out))
            out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
        xref = len(out)
        out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(self.objects) + 1)
        for o in offsets:
            out += b"%010d 00000 n \n" % o
        trailer = b"<< /Size %d /Root %d 0 R" % (len(self.objects) + 1, root)
        if info:
            trailer += b" /Info %d 0 R" % info
        if enc_num:
            trailer += b" /Encrypt %d 0 R" % enc_num
        trailer += b" /ID [<%s> <%s>] >>" % (doc_id.hex().encode(), doc_id.hex().encode())
        out += b"trailer\n" + trailer + b"\nstartxref\n%d\n%%%%EOF\n" % xref
        return bytes(out)

    @staticmethod
    def _encrypt_obj(body: bytes, num: int, key: bytes) -> bytes:
        k = hashlib.md5(key + num.to_bytes(3, "little") + b"\x00\x00").digest()[: len(key) + 5]
        if b"stream\n" in body:
            head, rest = body.split(b"stream\n", 1)
            data = rest[: rest.rindex(b"\nendstream")]
            return head + b"stream\n" + rc4(k, data) + b"\nendstream"
        # Literal-Strings (nur einfache, ohne Klammern im Inhalt) verschlüsseln
        out = bytearray()
        i = 0
        while i < len(body):
            if body[i : i + 1] == b"(":
                j = i + 1
                raw = bytearray()
                while body[j : j + 1] != b")":
                    if body[j : j + 1] == b"\\":
                        j += 1
                    raw += body[j : j + 1]
                    j += 1
                out += b"<" + rc4(k, bytes(raw)).hex().encode() + b">"
                i = j + 1
            else:
                out += body[i : i + 1]
                i += 1
        return bytes(out)


def make(path, pages, info=None, kennwort=None, shapes=False, extras=None):
    """pages: Liste (Breite, Höhe, Überschrift)."""
    w = Writer()
    font = w.add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    pages_num = w.add(b"")  # später
    kids = []
    total = len(pages)
    for n, (pw, ph, heading) in enumerate(pages, start=1):
        extra = (extras or {}).get(n, "")
        content = page_content(n, total, pw, ph, heading, extra, shapes)
        c = w.add(b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream")
        p = w.add(
            b"<< /Type /Page /Parent %d 0 R /MediaBox [0 0 %.2f %.2f] /Contents %d 0 R "
            b"/Resources << /Font << /F1 %d 0 R >> >> >>" % (pages_num, pw, ph, c, font)
        )
        kids.append(p)
    w.set(pages_num, b"<< /Type /Pages /Kids [%s] /Count %d >>" % (b" ".join(b"%d 0 R" % k for k in kids), len(kids)))
    root = w.add(b"<< /Type /Catalog /Pages %d 0 R >>" % pages_num)
    info_num = None
    if info:
        fields = b" ".join(b"/" + k.encode() + b" " + pdf_string(v) for k, v in info.items())
        info_num = w.add(b"<< " + fields + b" /CreationDate (D:20261020120000+02'00') /ModDate (D:20261020121500+02'00') >>")
    encrypt = None
    if kennwort is not None:
        def encrypt(doc_id):
            pw = (kennwort.encode("latin-1") + PAD)[:32]
            o_key = hashlib.md5(pw).digest()[:5]  # Besitzer = Benutzer
            o = rc4(o_key, pw)
            p = (-44).to_bytes(4, "little", signed=True)
            key = hashlib.md5(pw + o + p + doc_id).digest()[:5]
            u = rc4(key, PAD)
            body = b"<< /Filter /Standard /V 1 /R 2 /O <%s> /U <%s> /P -44 >>" % (o.hex().encode(), u.hex().encode())
            return key, body
    with open(path, "wb") as f:
        f.write(w.build(root, info_num, encrypt))


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    a4 = (595.28, 841.89)
    bericht = []
    for n in range(1, 13):
        if n == 5:
            bericht.append((a4[1], a4[0], f"Kapitel {n}: Querformat"))
        else:
            bericht.append((a4[0], a4[1], f"Kapitel {n}: Querencia Testbericht"))
    make(
        os.path.join(out, "bericht.pdf"),
        bericht,
        info={
            "Title": "Querencia Testbericht",
            "Author": "Querencia Desktop contributors",
            "Subject": "Testdokument für Querencia Dokumente",
            "Keywords": "Test, PDF, Querencia",
            "Creator": "werkzeug/testpdf.py",
            "Producer": "Querencia-Testgenerator",
        },
        extras={3: "Sonderwort Zwetschgenkuchen steht nur hier.", 9: "Noch einmal Zwetschgenkuchen."},
    )
    make(os.path.join(out, "brief.pdf"), [(612.0, 792.0, "Ein Brief im US-Letter-Format")])
    make(
        os.path.join(out, "gross-500.pdf"),
        [(a4[0], a4[1], f"Abschnitt {n}") for n in range(1, 501)],
        info={"Title": "Großes Testdokument", "Author": "Querencia Desktop contributors"},
        shapes=True,
        extras={n: "Seltenes Suchwort Leuchtturm." for n in (7, 123, 250, 499)},
    )
    make(
        os.path.join(out, "geschuetzt.pdf"),
        [(a4[0], a4[1], f"Geschützte Seite {n}") for n in range(1, 4)],
        info={"Title": "Geschütztes Testdokument"},
        # öffentlich bekanntes Testkennwort (Unit-Tests, GUI-Szenario)
        kennwort="querencia",
    )
    with open(os.path.join(out, "bericht.pdf"), "rb") as f:
        data = f.read()
    with open(os.path.join(out, "kaputt.pdf"), "wb") as f:
        f.write(data[:300])
    for name in sorted(os.listdir(out)):
        print(name, os.path.getsize(os.path.join(out, name)))


if __name__ == "__main__":
    main()
