#!/usr/bin/env python3
"""Dokumente 0.1.3: Leerseiten in Bildschirmfotos der Hauptansicht finden.

Die Fläche hinter den Seiten ist #8a8580 (style.css), Seiten sind weiß.
Je Bild: Ansichtsbereich = Zeilen und Spalten mit diesem Grau; je Zeile die
Strecke vom ersten bis zum letzten weißen Pixel; zusammenhängende Zeilen mit
Strecke bilden eine Seite (Lücken zwischen Seiten sind grau). Je Seite der
Anteil dunkler Pixel (alle Kanäle < 160, ohne die äußeren 3 Pixel: Rahmen).
LEER: Seite ≥ 200 Zeilen hoch und Anteil < 0,0005.

Aufruf: leerseiten.py <bild.png|ordner> …   (Exit 1, wenn eine Seite LEER ist)
Ohne numpy/PIL (nicht auf der VM): GdkPixbuf und Ganzzahl-Bitmasken.
"""

import os
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf  # noqa: E402

GRAU = (0x8A, 0x85, 0x80)
TOL = 4
MIN_HOEHE = 200
SCHWELLE = 0.0005

WEISS = bytes(1 if b >= 250 else 0 for b in range(256))
DUNKEL = bytes(1 if b < 160 else 0 for b in range(256))
GRAUT = [bytes(1 if abs(b - g) <= TOL else 0 for b in range(256)) for g in GRAU]


def maske(kanaele, tabellen, und):
    """Bitmaske je Pixel (ein Bit je Byte-Stelle) über alle drei Kanäle."""
    m = None
    for k, t in zip(kanaele, tabellen):
        v = int.from_bytes(k.translate(t), "big")
        m = v if m is None else (m & v if und else m | v)
    return m


def zeilen(pb):
    w, h = pb.get_width(), pb.get_height()
    nc, rs = pb.get_n_channels(), pb.get_rowstride()
    px = pb.get_pixels()
    for y in range(h):
        row = px[y * rs:y * rs + w * nc]
        yield y, w, (row[0::nc], row[1::nc], row[2::nc])


def stellen(m, w):
    """Erste und letzte gesetzte Byte-Stelle (Pixel-x) einer Maske."""
    if not m:
        return None
    last = w - 1 - ((m & -m).bit_length() - 1) // 8
    first = w - 1 - (m.bit_length() - 1) // 8
    return first, last


def einzaehlen(m):
    return bin(m).count("1")


def auswerten(pfad):
    pb = GdkPixbuf.Pixbuf.new_from_file(pfad)
    w = pb.get_width()
    grau = []
    daten = []
    for y, w, k in zeilen(pb):
        g = maske(k, GRAUT, True)
        grau.append(g if einzaehlen(g) >= 5 else 0)
        daten.append(k)
    # Ansicht = längste Folge von Zeilen mit Grau (Lücken bis 8 Zeilen);
    # einzelne graue Kantenpixel in Menü oder Leiste (dunkles Thema) zählen
    # so nicht.
    beste, start, letzte = (0, -1, -1), None, None
    for y, g in enumerate(grau):
        if not g:
            continue
        if letzte is None or y - letzte > 9:
            start = y
        letzte = y
        if letzte - start > beste[0]:
            beste = (letzte - start, start, letzte)
    if beste[1] < 0:
        return None, []
    # Spalten der Ansicht: in mindestens 30 % der Zeilen grau (jede vierte
    # Zeile gezählt); vereinzelte Treffer in der Seitenleiste fallen weg.
    zaehler = [0] * w
    proben = range(beste[1], beste[2] + 1, 4)
    for y in proben:
        for i, b in enumerate(grau[y].to_bytes(w, "big")):
            zaehler[i] += b
    grau_x = int.from_bytes(bytes(1 if z >= 0.3 * len(proben) else 0 for z in zaehler), "big")
    if not grau_x:
        return None, []
    x0, x1 = stellen(grau_x, w)
    breite_bits = x1 - x0 + 1
    fenster = ((1 << (8 * breite_bits)) - 1) << (8 * (w - 1 - x1))
    # Unten endet die Ansicht an der ersten Zeile ohne Grau und ohne Weiß
    # (eine Seite kann breiter als die Ansicht sein: dann fehlt das Grau).
    # Rahmenzeilen (dunkel) zählen mit; Schluss nach 8 Zeilen ohne beides.
    y0 = y1 = beste[1]
    for y in range(y0, len(daten)):
        k = daten[y]
        if maske(k, GRAUT, True) & fenster or maske(k, (WEISS,) * 3, True) & fenster:
            y1 = y
        elif y - y1 > 8:
            break
    seiten = []
    akt = None
    for y in range(y0, y1 + 1):
        k = daten[y]
        weiss = maske(k, (WEISS,) * 3, True) & fenster
        s = stellen(weiss, w)
        if s and s[1] - s[0] >= 50:
            a, b = s[0] + 3, s[1] - 3
            innen = ((1 << (8 * (b - a + 1))) - 1) << (8 * (w - 1 - b))
            dunkel = einzaehlen(maske(k, (DUNKEL,) * 3, True) & innen)
            if akt is None:
                akt = {"y0": y, "y1": y, "x0": s[0], "x1": s[1], "px": 0, "dunkel": 0}
                seiten.append(akt)
            akt["y1"] = y
            akt["x0"] = min(akt["x0"], s[0])
            akt["x1"] = max(akt["x1"], s[1])
            akt["px"] += b - a + 1
            akt["dunkel"] += dunkel
        else:
            akt = None
    for s in seiten:
        s["h"] = s["y1"] - s["y0"] + 1
        s["anteil"] = s["dunkel"] / s["px"] if s["px"] else 0.0
        s["leer"] = s["h"] >= MIN_HOEHE and s["anteil"] < SCHWELLE
    return (x0, y0, x1, y1), seiten


def main():
    bilder = []
    for a in sys.argv[1:]:
        if os.path.isdir(a):
            bilder += sorted(os.path.join(a, f) for f in os.listdir(a) if f.endswith(".png"))
        else:
            bilder.append(a)
    leer = 0
    for b in bilder:
        bereich, seiten = auswerten(b)
        teile = " ".join(
            f"[y{s['y0']}-{s['y1']} h{s['h']} {s['anteil']:.4f}{' LEER' if s['leer'] else ''}]"
            for s in seiten)
        urteil = "LEER" if any(s["leer"] for s in seiten) else ("ohne-ansicht" if bereich is None else "ok")
        leer += urteil == "LEER"
        print(f"{os.path.basename(b)}\t{urteil}\t{bereich}\t{teile}")
    print(f"Bilder {len(bilder)}, mit Leerseite {leer}")
    return 1 if leer else 0


if __name__ == "__main__":
    sys.exit(main())
