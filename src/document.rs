//! Ein geöffnetes PDF im Hauptfaden: Seitengrößen, Metadaten, Textauswahl.
//!
//! Poppler-Dokumente sind nicht fadensicher. Jeder Faden öffnet deshalb sein
//! eigenes `poppler::Document` aus derselben Datei (dieses hier für Auswahl,
//! Eigenschaften und Drucken; render.rs für Bilder und Suche).

use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::layout::Rect;
use crate::selection::Selection;

/// Warum ein Dokument nicht geöffnet werden konnte.
#[derive(Clone, Debug, PartialEq)]
pub enum OpenError {
    /// Verschlüsselt, noch kein Kennwort angegeben.
    NeedsPassword,
    /// Verschlüsselt, das Kennwort stimmt nicht.
    WrongPassword,
    /// Datei fehlt oder ist nicht lesbar (Meldung von Poppler).
    Unreadable(String),
    /// Defekt oder kein PDF (Meldung von Poppler).
    Damaged(String),
}

/// URI für Poppler (`file:///…`, Sonderzeichen maskiert).
pub fn uri_of(path: &Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

/// Öffnet `path` mit Poppler (in jedem Faden verwendbar).
pub fn open(path: &Path, password: Option<&str>) -> Result<poppler::Document, OpenError> {
    poppler::Document::from_file(&uri_of(path), password).map_err(|err| {
        match err.kind::<poppler::Error>() {
            Some(poppler::Error::Encrypted) if password.is_some() => OpenError::WrongPassword,
            Some(poppler::Error::Encrypted) => OpenError::NeedsPassword,
            Some(poppler::Error::OpenFile) => OpenError::Unreadable(err.message().to_string()),
            // fehlende Datei, keine Rechte: Fehler aus GLib statt Poppler
            _ if err.kind::<glib::FileError>().is_some() => {
                OpenError::Unreadable(err.message().to_string())
            }
            _ => OpenError::Damaged(err.message().to_string()),
        }
    })
}

pub struct Doc {
    pub path: PathBuf,
    pub password: Option<String>,
    pub poppler: poppler::Document,
    /// Seitengrößen in Punkt.
    pub sizes: Vec<(f64, f64)>,
}

impl Doc {
    pub fn load(path: &Path, password: Option<&str>) -> Result<Self, OpenError> {
        let poppler = open(path, password)?;
        let n = poppler.n_pages().max(0);
        let sizes: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                poppler
                    .page(i)
                    .map(|p| p.size())
                    .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                    // Seite ohne gültige Größe: als A4 anzeigen (leer)
                    .unwrap_or((595.0, 842.0))
            })
            .collect();
        if sizes.is_empty() {
            return Err(OpenError::Damaged(String::new()));
        }
        Ok(Self {
            path: path.to_path_buf(),
            password: password.map(str::to_string),
            poppler,
            sizes,
        })
    }

    pub fn n_pages(&self) -> usize {
        self.sizes.len()
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Titel aus den Metadaten, wenn er nicht leer ist.
    pub fn title(&self) -> Option<String> {
        self.poppler
            .title()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    }

    /// Seitennummer (0-basiert) zu einer Seitenbezeichnung („iv“, „A-3“).
    pub fn page_for_label(&self, label: &str) -> Option<usize> {
        let page = self.poppler.page_by_label(label)?;
        usize::try_from(page.index()).ok()
    }

    fn rectangle(area: (f64, f64, f64, f64)) -> poppler::Rectangle {
        let mut r = poppler::Rectangle::new();
        r.set_x1(area.0);
        r.set_y1(area.1);
        r.set_x2(area.2);
        r.set_y2(area.3);
        r
    }

    /// Markierter Text über alle Seiten der Auswahl, Seiten durch
    /// Zeilenumbruch getrennt.
    pub fn selected_text(&self, sel: &Selection) -> String {
        let mut out = String::new();
        for page in sel.pages() {
            let Some(area) = sel.area_on(page, self.sizes[page]) else {
                continue;
            };
            let Some(p) = self.poppler.page(page as i32) else {
                continue;
            };
            let mut r = Self::rectangle(area);
            if let Some(text) = p.selected_text(poppler::SelectionStyle::Glyph, &mut r) {
                if !out.is_empty() && !out.ends_with('\n') && !text.is_empty() {
                    out.push('\n');
                }
                out.push_str(&text);
            }
        }
        out
    }

    /// Rechtecke der Auswahl auf `page` in Punkt (Ursprung oben links).
    pub fn selection_rects(&self, page: usize, sel: &Selection) -> Vec<Rect> {
        let Some(area) = self
            .sizes
            .get(page)
            .and_then(|size| sel.area_on(page, *size))
        else {
            return Vec::new();
        };
        let Some(p) = self.poppler.page(page as i32) else {
            return Vec::new();
        };
        // Region in ganzen Pixeln: mit 4 je Punkt rechnen, dann zurückteilen.
        const S: f64 = 4.0;
        let mut r = Self::rectangle(area);
        let Some(region) = p.selected_region(S, poppler::SelectionStyle::Glyph, &mut r) else {
            return Vec::new();
        };
        (0..region.num_rectangles())
            .map(|i| {
                let q = region.rectangle(i);
                Rect::new(
                    f64::from(q.x()) / S,
                    f64::from(q.y()) / S,
                    f64::from(q.width()) / S,
                    f64::from(q.height()) / S,
                )
            })
            .collect()
    }

    pub fn properties(&self) -> Properties {
        let d = &self.poppler;
        let text =
            |s: Option<glib::GString>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Properties {
            title: text(d.title()),
            subject: text(d.subject()),
            author: text(d.author()),
            keywords: text(d.keywords()),
            creator: text(d.creator()),
            producer: text(d.producer()),
            created: d.creation_date_time(),
            modified: d.modification_date_time(),
            version: text(d.pdf_version_string()),
            pages: self.n_pages(),
            page_size: self.sizes[0],
            encrypted: self.password.is_some(),
        }
    }
}

/// Metadaten für „Eigenschaften“ (properties.rs formatiert sie).
pub struct Properties {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub created: Option<glib::DateTime>,
    pub modified: Option<glib::DateTime>,
    pub version: Option<String>,
    pub pages: usize,
    /// Größe der ersten Seite in Punkt.
    pub page_size: (f64, f64),
    pub encrypted: bool,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Zweiseitiges Test-PDF (A4, dann quer) mit Text über Cairo.
    pub fn sample_pdf(dir: &Path) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("probe.pdf");
        let surface = cairo::PdfSurface::new(595.0, 842.0, &path).unwrap();
        surface
            .set_metadata(cairo::PdfMetadata::Title, "Probe")
            .unwrap();
        surface
            .set_metadata(cairo::PdfMetadata::Author, "Querencia Desktop contributors")
            .unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
            cr.set_font_size(20.0);
            cr.move_to(50.0, 100.0);
            cr.show_text("Querencia Dokumente erste Seite").unwrap();
            cr.show_page().unwrap();
            surface.set_size(842.0, 595.0).unwrap();
            cr.move_to(50.0, 500.0);
            cr.show_text("Zweite Seite mit Querencia unten").unwrap();
            cr.show_page().unwrap();
        }
        surface.finish();
        path
    }

    pub fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("qd-{name}-{}", std::process::id()))
    }

    #[test]
    fn a_pdf_opens_with_sizes_and_metadata() {
        let dir = temp("doc");
        let doc = Doc::load(&sample_pdf(&dir), None).unwrap();
        assert_eq!(doc.n_pages(), 2);
        assert_eq!(doc.sizes, vec![(595.0, 842.0), (842.0, 595.0)]);
        assert_eq!(doc.title().as_deref(), Some("Probe"));
        let p = doc.properties();
        assert_eq!(p.author.as_deref(), Some("Querencia Desktop contributors"));
        assert_eq!(p.pages, 2);
        assert!(!p.encrypted);
        assert_eq!(doc.page_for_label("2"), Some(1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn selected_text_spans_pages_and_has_rectangles() {
        let dir = temp("sel");
        let doc = Doc::load(&sample_pdf(&dir), None).unwrap();
        let all = Selection::all((1, 842.0, 595.0));
        let text = doc.selected_text(&all);
        assert!(text.contains("Querencia Dokumente erste Seite"), "{text:?}");
        assert!(
            text.contains("Zweite Seite mit Querencia unten"),
            "{text:?}"
        );
        let rects = doc.selection_rects(0, &all);
        assert!(!rects.is_empty());
        // die Zeile steht bei y ≈ 100 pt (Grundlinie) von oben
        assert!(
            rects.iter().any(|r| r.y < 100.0 && r.y + r.h > 85.0),
            "{rects:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn broken_missing_and_locked_files_are_told_apart() {
        let dir = temp("err");
        std::fs::create_dir_all(&dir).unwrap();
        let broken = dir.join("kaputt.pdf");
        std::fs::write(&broken, b"%PDF-1.4\nkein echtes PDF").unwrap();
        assert!(matches!(
            Doc::load(&broken, None),
            Err(OpenError::Damaged(_))
        ));
        assert!(matches!(
            Doc::load(&dir.join("fehlt.pdf"), None),
            Err(OpenError::Unreadable(_))
        ));
        // RC4-verschlüsselt, Passwort „querencia“ (werkzeug/testpdf.py)
        let locked = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdaten/geschuetzt.pdf");
        assert_eq!(
            Doc::load(&locked, None).err(),
            Some(OpenError::NeedsPassword)
        );
        assert_eq!(
            Doc::load(&locked, Some("falsch")).err(),
            Some(OpenError::WrongPassword)
        );
        let doc = Doc::load(&locked, Some("querencia")).unwrap();
        assert_eq!(doc.n_pages(), 3);
        assert!(doc.properties().encrypted);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
