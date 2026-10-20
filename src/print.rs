//! Drucken über `gtk::PrintOperation`: jede Seite mit Poppler
//! (`render_for_printing`) auf die Druckseite eingepasst, zentriert und bei
//! abweichender Ausrichtung gedreht (wie Evince). Die Druckeinstellungen
//! bleiben je Fenster für den nächsten Druck erhalten.

use std::cell::RefCell;
use std::f64::consts::FRAC_PI_2;
use std::rc::Rc;

use gtk::prelude::*;

use crate::dialogs;
use crate::document::Doc;
use crate::i18n::{gettext, gettext_f};
use crate::properties;

/// Maßstab und Drehung, um eine Seite `page` (Punkt) in `area` (Einheiten
/// des Druckkontexts) einzupassen: (Faktor, gedreht).
pub fn fit(page: (f64, f64), area: (f64, f64)) -> (f64, bool) {
    let (pw, ph) = page;
    let (aw, ah) = area;
    if pw <= 0.0 || ph <= 0.0 || aw <= 0.0 || ah <= 0.0 {
        return (1.0, false);
    }
    let rotate = (pw > ph) != (aw > ah) && (pw - ph).abs() > 1.0 && (aw - ah).abs() > 1.0;
    let (pw, ph) = if rotate { (ph, pw) } else { (pw, ph) };
    ((aw / pw).min(ah / ph), rotate)
}

/// Name des Papierformats für GTK (PWG 5101.1).
pub fn paper_ppd(name: &str) -> Option<&'static str> {
    Some(match name {
        "A3" => "iso_a3",
        "A4" => "iso_a4",
        "A5" => "iso_a5",
        "A6" => "iso_a6",
        "B5" => "iso_b5",
        "US Letter" => "na_letter",
        "US Legal" => "na_legal",
        _ => return None,
    })
}

pub fn run(
    parent: &gtk::ApplicationWindow,
    doc: Rc<Doc>,
    settings: Rc<RefCell<Option<gtk::PrintSettings>>>,
    error_window: gtk::Window,
) {
    let op = gtk::PrintOperation::new();
    op.set_n_pages(doc.n_pages() as i32);
    op.set_job_name(&doc.file_name());
    op.set_embed_page_setup(true);
    // kein verschachtelter Hauptschleifen-Lauf
    op.set_allow_async(true);
    let s = settings.borrow().clone().unwrap_or_else(|| {
        let s = gtk::PrintSettings::new();
        // „In Datei drucken“ schlägt den Dokumentnamen vor statt „output“
        let stem = doc
            .path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !stem.is_empty() {
            s.set(gtk::PRINT_SETTINGS_OUTPUT_BASENAME, Some(&stem));
        }
        s
    });
    op.set_print_settings(Some(&s));
    // Papier wie das Dokument, wenn es ein Normformat ist (A4-Dokument → A4,
    // auch in einer Sitzung mit LANG=en_US, die sonst US Letter vorgibt)
    if settings.borrow().is_none()
        && let Some(name) = doc
            .sizes
            .first()
            .and_then(|(w, h)| properties::paper_name(*w, *h))
            .and_then(|(name, _)| paper_ppd(name))
    {
        let setup = gtk::PageSetup::new();
        setup.set_paper_size(&gtk::PaperSize::new(Some(name)));
        op.set_default_page_setup(Some(&setup));
    }
    let d = doc.clone();
    op.connect_draw_page(move |_, ctx, n| {
        let Some(page) = d.poppler.page(n) else {
            return;
        };
        let cr = ctx.cairo_context();
        let size = page.size();
        let (aw, ah) = (ctx.width(), ctx.height());
        let (f, rotate) = fit(size, (aw, ah));
        let (w, h) = if rotate {
            (size.1 * f, size.0 * f)
        } else {
            (size.0 * f, size.1 * f)
        };
        cr.translate((aw - w) / 2.0, (ah - h) / 2.0);
        if rotate {
            cr.translate(w, 0.0);
            cr.rotate(FRAC_PI_2);
        }
        cr.scale(f, f);
        page.render_for_printing(&cr);
    });
    let name = doc.file_name();
    let errors = error_window.clone();
    op.connect_done(move |op, result| match result {
        gtk::PrintOperationResult::Apply => {
            settings.replace(op.print_settings());
        }
        gtk::PrintOperationResult::Error => {
            let detail = op
                .error()
                .map(|e| e.message().to_string())
                .unwrap_or_default();
            dialogs::show_error(
                &errors,
                &gettext_f("Failed to print document “{name}”.", &[("name", &name)]),
                &detail,
            );
        }
        _ => {}
    });
    if let Err(err) = op.run(gtk::PrintOperationAction::PrintDialog, Some(parent)) {
        dialogs::show_error(
            &error_window,
            &gettext("Failed to print document."),
            err.message(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_papers_map_to_gtk_names() {
        assert_eq!(paper_ppd("A4"), Some("iso_a4"));
        assert_eq!(paper_ppd("US Letter"), Some("na_letter"));
        assert_eq!(paper_ppd("Tabloid"), None);
    }

    #[test]
    fn pages_are_fitted_and_rotated_like_evince() {
        // A4 auf A4 (Druckfläche in Punkt): Faktor 1
        assert_eq!(fit((595.0, 842.0), (595.0, 842.0)), (1.0, false));
        // A4 auf kleinere Druckfläche (Ränder)
        let (f, r) = fit((595.0, 842.0), (540.0, 780.0));
        assert!(!r && (f - (540.0_f64 / 595.0).min(780.0 / 842.0)).abs() < 1e-12);
        // Querformat auf Hochformat: gedreht und eingepasst
        let (f, r) = fit((842.0, 595.0), (595.0, 842.0));
        assert!(r && (f - 1.0).abs() < 1e-12);
        // quadratisch: nie drehen
        assert!(!fit((500.0, 500.0), (595.0, 842.0)).1);
        assert_eq!(fit((0.0, 1.0), (1.0, 1.0)), (1.0, false));
    }
}
