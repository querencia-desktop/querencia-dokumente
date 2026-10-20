//! „Properties“ („Eigenschaften“): Metadaten des Dokuments wie in Evince 2.30
//! (Titel, Ort, Thema, Autor, Stichwörter, Ersteller, Erzeugt mit, Datum,
//! Seitenzahl, Format, Sicherheit, Papiergröße) und die Dateigröße.
//! Ein gewöhnliches Fenster (labwc-Dekoration), Escape schließt; es wird bei
//! jedem Öffnen neu gebaut und beim Schließen zerstört (QD-076, siehe
//! dialogs.rs).

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::dialogs;
use crate::document::{Doc, Properties};
use crate::i18n::{self, gettext, gettext_f, ngettext_f, pgettext};

const MM_PER_PT: f64 = 25.4 / 72.0;

/// Normformate in mm (Hochformat), Toleranz 2 mm.
const PAPERS: [(&str, f64, f64); 12] = [
    ("A0", 841.0, 1189.0),
    ("A1", 594.0, 841.0),
    ("A2", 420.0, 594.0),
    ("A3", 297.0, 420.0),
    ("A4", 210.0, 297.0),
    ("A5", 148.0, 210.0),
    ("A6", 105.0, 148.0),
    ("B4", 250.0, 353.0),
    ("B5", 176.0, 250.0),
    ("US Letter", 215.9, 279.4),
    ("US Legal", 215.9, 355.6),
    ("Tabloid", 279.4, 431.8),
];

/// Name des Papierformats und ob es quer liegt.
pub fn paper_name(w_pt: f64, h_pt: f64) -> Option<(&'static str, bool)> {
    let (w, h) = (w_pt * MM_PER_PT, h_pt * MM_PER_PT);
    let (short, long) = (w.min(h), w.max(h));
    PAPERS
        .iter()
        .find(|(_, pw, ph)| (short - pw).abs() <= 2.0 && (long - ph).abs() <= 2.0)
        .map(|(name, _, _)| (*name, w > h))
}

/// „210 × 297 mm (A4)“, „297 × 210 mm (A4, landscape)“, „100 × 100 mm“.
pub fn paper_size(w_pt: f64, h_pt: f64) -> String {
    let mm = |v: f64| format!("{:.0}", v * MM_PER_PT);
    let (w, h) = (mm(w_pt), mm(h_pt));
    match paper_name(w_pt, h_pt) {
        Some((name, false)) => gettext_f(
            // TRANSLATORS: Papiergröße mit Formatname, z. B. „210 × 297 mm (A4)“
            "{width} × {height} mm ({paper})",
            &[("width", &w), ("height", &h), ("paper", name)],
        ),
        Some((name, true)) => gettext_f(
            // TRANSLATORS: Papiergröße eines quer liegenden Normformats
            "{width} × {height} mm ({paper}, landscape)",
            &[("width", &w), ("height", &h), ("paper", name)],
        ),
        None => gettext_f("{width} × {height} mm", &[("width", &w), ("height", &h)]),
    }
}

/// Dateigröße: „2,9 kB“, „1,4 MB“ im Deutschen, „2.9 kB“ im Englischen
/// (Basis 1000 wie GNOME; Dezimalzeichen aus dem Katalog).
pub fn human_size(bytes: u64) -> String {
    size_with(bytes, &i18n::decimal_separator())
}

fn size_with(bytes: u64, sep: &str) -> String {
    let b = bytes as f64;
    if b < 1000.0 {
        ngettext_f("{n} byte", "{n} bytes", bytes as u32, &[])
    } else if b < 1e6 {
        format!("{} kB", i18n::decimal(b / 1e3, 1, sep))
    } else if b < 1e9 {
        format!("{} MB", i18n::decimal(b / 1e6, 1, sep))
    } else {
        format!("{} GB", i18n::decimal(b / 1e9, 1, sep))
    }
}

fn date(d: &Option<glib::DateTime>) -> Option<String> {
    d.as_ref()
        .and_then(|dt| dt.to_local().ok())
        // TRANSLATORS: Datum und Uhrzeit, Format von g_date_time_format
        // (%d Tag, %m Monat, %Y Jahr, %H:%M Uhrzeit)
        .and_then(|dt| dt.format(&gettext("%Y-%m-%d %H:%M")).ok())
        .map(|s| s.to_string())
}

/// Zeilen des Fensters; leere Angaben entfallen (außer Titel).
pub fn rows(doc: &Doc, p: &Properties) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let none = || gettext("None");
    rows.push((gettext("Title"), p.title.clone().unwrap_or_else(none)));
    let file = gio::File::for_path(&doc.path);
    if let Some(dir) = file.parent() {
        rows.push((gettext("Location"), dir.parse_name().to_string()));
    }
    rows.push((gettext("File"), doc.file_name()));
    for (label, value) in [
        (gettext("Subject"), &p.subject),
        (gettext("Author"), &p.author),
        (gettext("Keywords"), &p.keywords),
        (gettext("Creator"), &p.creator),
        (gettext("Producer"), &p.producer),
    ] {
        if let Some(v) = value {
            rows.push((label, v.clone()));
        }
    }
    if let Some(s) = date(&p.created) {
        rows.push((gettext("Created"), s));
    }
    if let Some(s) = date(&p.modified) {
        rows.push((gettext("Modified"), s));
    }
    rows.push((gettext("Number of Pages"), p.pages.to_string()));
    if let Some(v) = &p.version {
        rows.push((gettext("Format"), v.clone()));
    }
    rows.push((
        gettext("Security"),
        if p.encrypted {
            // TRANSLATORS: Eigenschaften, Zeile „Sicherheit“ bei Kennwortschutz
            pgettext("security", "Password protected")
        } else {
            none()
        },
    ));
    rows.push((
        gettext("Paper Size"),
        paper_size(p.page_size.0, p.page_size.1),
    ));
    if let Ok(info) = file.query_info(
        "standard::size",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) {
        rows.push((gettext("File Size"), human_size(info.size().max(0) as u64)));
    }
    rows
}

/// Zeigt die Eigenschaften im offenen Fenster von `slot` (neuer Inhalt) oder
/// in einem neu gebauten.
pub fn show(slot: &dialogs::Slot, doc: &Doc) {
    let Some(window) = slot.window() else { return };
    let window = &window;
    let rows = rows(doc, &doc.properties());
    let grid = gtk::Grid::builder()
        .row_spacing(4)
        .column_spacing(12)
        .build();
    for (i, (k, v)) in rows.iter().enumerate() {
        let key = gtk::Label::new(None);
        // TRANSLATORS: Bezeichnung einer Zeile in den Eigenschaften, fett
        let k = gettext_f("{label}:", &[("label", k)]);
        key.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&k)));
        key.set_xalign(1.0);
        key.set_valign(gtk::Align::Start);
        let val = gtk::Label::new(Some(v));
        val.set_xalign(0.0);
        val.set_selectable(true);
        // lange Werte (Ort, Stichwörter) in der Mitte kürzen, ganz im
        // Tooltip; kein Umbruch (umbrechende Zeilen ließen GTK das feste
        // Fenster zu niedrig messen)
        val.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        val.set_width_chars(24);
        val.set_max_width_chars(56);
        val.set_tooltip_text(Some(v));
        grid.attach(&key, 0, i as i32, 1, 1);
        grid.attach(&val, 1, i as i32, 1, 1);
    }
    window.set_title(Some(&gettext("Properties")));
    dialogs::set_content(window, &dialogs::with_close_button(window, &grid));
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_names_ignore_orientation_and_rounding() {
        assert_eq!(paper_name(595.0, 842.0), Some(("A4", false)));
        assert_eq!(paper_name(595.28, 841.89), Some(("A4", false)));
        assert_eq!(paper_name(842.0, 595.0), Some(("A4", true)));
        assert_eq!(paper_name(612.0, 792.0), Some(("US Letter", false)));
        assert_eq!(paper_name(612.0, 1008.0), Some(("US Legal", false)));
        assert_eq!(paper_name(420.0, 595.0), Some(("A5", false)));
        assert_eq!(paper_name(300.0, 300.0), None);
    }

    #[test]
    fn paper_sizes_are_written_in_millimetres() {
        assert_eq!(paper_size(595.0, 842.0), "210 × 297 mm (A4)");
        assert_eq!(paper_size(842.0, 595.0), "297 × 210 mm (A4, landscape)");
        assert_eq!(paper_size(612.0, 792.0), "216 × 279 mm (US Letter)");
        assert_eq!(paper_size(283.46, 283.46), "100 × 100 mm");
    }

    #[test]
    fn sizes_use_the_decimal_separator_of_the_catalog() {
        assert_eq!(size_with(2920, ","), "2,9 kB");
        assert_eq!(size_with(1_478_886, ","), "1,5 MB");
        assert_eq!(human_size(19), "19 bytes");
        assert_eq!(human_size(1), "1 byte");
        assert_eq!(human_size(2920), "2.9 kB");
    }
}
