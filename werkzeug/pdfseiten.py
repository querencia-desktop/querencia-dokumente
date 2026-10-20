#!/usr/bin/env python3
"""Seitenzahl eines PDFs über Poppler (gir1.2-poppler), für die Druckprobe.
Aufruf: pdfseiten.py <datei>"""
import os
import sys

import gi

gi.require_version("Poppler", "0.18")
from gi.repository import Gio, Poppler  # noqa: E402

uri = Gio.File.new_for_path(os.path.abspath(sys.argv[1])).get_uri()
print(Poppler.Document.new_from_file(uri, None).get_n_pages())
