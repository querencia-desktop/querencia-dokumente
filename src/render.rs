//! Hintergrundfäden je Dokument: einer rendert Seiten und Miniaturen, einer
//! sucht Text. Beide öffnen ihr eigenes Poppler-Dokument (Poppler ist nicht
//! fadensicher je Dokument) und melden ihre Ergebnisse über einen Kanal an
//! die GTK-Hauptschleife.
//!
//! Die Warteschlange des Renderfadens ersetzt der Hauptfaden bei jeder
//! Änderung der Ansicht vollständig (sichtbare Seiten zuerst, dann Nachbarn,
//! dann Miniaturen). Nicht mehr gebrauchte Aufträge entfallen so von selbst;
//! große Dokumente blockieren nie die Oberfläche.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use futures_channel::mpsc::UnboundedSender;
use gtk::glib;

use crate::document;
use crate::layout::Rect;
use crate::search::from_pdf_rect;

/// Höchstens so viele Pixel je Bild (≈ 96 MB bei 4 Byte je Pixel); größere
/// Seiten (Poster bei 400 %) werden kleiner gerendert und hochskaliert.
pub const MAX_PIXELS: f64 = 24.0e6;
/// Höchste Kantenlänge eines Bildes (Grenze der GPU-Texturen).
pub const MAX_EDGE: f64 = 8192.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Job {
    pub page: usize,
    /// Pixel je Punkt in Tausendsteln (cache::scale_key).
    pub key: u32,
    /// Miniatur für die Seitenleiste statt Seite der Ansicht.
    pub thumb: bool,
}

/// Gerenderte Seite: Pixel im Cairo-Format ARGB32 (vormultipliziert, in
/// Speicherreihenfolge B, G, R, A).
pub struct Image {
    pub job: Job,
    pub width: i32,
    pub height: i32,
    pub stride: usize,
    pub data: glib::Bytes,
}

pub enum Message {
    Rendered(Image),
    Failed(Job),
    /// Suchergebnis einer Seite (auch leer: Fortschritt).
    Found {
        generation: u64,
        page: usize,
        rects: Vec<Rect>,
    },
    SearchDone {
        generation: u64,
    },
}

/// Bildgröße für eine Seite von `size_pt` bei `scale` (Pixel je Punkt), mit
/// den Grenzen [`MAX_PIXELS`] und [`MAX_EDGE`].
pub fn image_size(size_pt: (f64, f64), scale: f64) -> (i32, i32) {
    let (mut w, mut h) = (size_pt.0 * scale, size_pt.1 * scale);
    if w * h > MAX_PIXELS {
        let f = (MAX_PIXELS / (w * h)).sqrt();
        w *= f;
        h *= f;
    }
    let edge = w.max(h);
    if edge > MAX_EDGE {
        w *= MAX_EDGE / edge;
        h *= MAX_EDGE / edge;
    }
    (w.round().max(1.0) as i32, h.round().max(1.0) as i32)
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    current: Option<Job>,
}

struct SearchRequest {
    generation: u64,
    query: String,
    case_sensitive: bool,
    start: usize,
}

#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    search: Mutex<Option<SearchRequest>>,
    search_wake: Condvar,
    search_generation: AtomicU64,
    stop: AtomicBool,
}

pub struct Renderer {
    shared: Arc<Shared>,
}

impl Renderer {
    pub fn start(path: PathBuf, password: Option<String>, tx: UnboundedSender<Message>) -> Self {
        let shared = Arc::new(Shared::default());
        {
            let (shared, path, password, tx) =
                (shared.clone(), path.clone(), password.clone(), tx.clone());
            std::thread::Builder::new()
                .name("qd-render".into())
                .spawn(move || render_loop(&shared, &path, password.as_deref(), &tx))
                .expect("Renderfaden");
        }
        {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("qd-search".into())
                .spawn(move || search_loop(&shared, &path, password.as_deref(), &tx))
                .expect("Suchfaden");
        }
        Self { shared }
    }

    /// Ersetzt die Warteschlange. Ein Auftrag, der gerade läuft, wird nicht
    /// noch einmal eingereiht.
    pub fn set_jobs(&self, jobs: Vec<Job>) {
        let mut q = self.shared.queue.lock().unwrap();
        let current = q.current;
        q.jobs = jobs.into_iter().filter(|j| Some(*j) != current).collect();
        if !q.jobs.is_empty() {
            self.shared.wake.notify_one();
        }
    }

    /// Startet eine neue Suche ab Seite `start` (ringsum) und bricht eine
    /// laufende ab. Gibt die Kennung der Suche zurück.
    pub fn search(&self, query: &str, case_sensitive: bool, start: usize) -> u64 {
        let generation = self.shared.search_generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.shared.search.lock().unwrap() = Some(SearchRequest {
            generation,
            query: query.to_string(),
            case_sensitive,
            start,
        });
        self.shared.search_wake.notify_one();
        generation
    }

    pub fn cancel_search(&self) {
        self.shared.search_generation.fetch_add(1, Ordering::SeqCst);
        self.shared.search.lock().unwrap().take();
    }
}

impl Drop for Renderer {
    /// Fäden beenden sich nach dem laufenden Auftrag; nicht warten, damit das
    /// Schließen nie an einer großen Seite hängt.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.search_generation.fetch_add(1, Ordering::SeqCst);
        self.shared.wake.notify_all();
        self.shared.search_wake.notify_all();
    }
}

fn render_loop(
    shared: &Shared,
    path: &std::path::Path,
    password: Option<&str>,
    tx: &UnboundedSender<Message>,
) {
    let Ok(doc) = document::open(path, password) else {
        return;
    };
    loop {
        let job = {
            let mut q = shared.queue.lock().unwrap();
            q.current = None;
            loop {
                if shared.stop.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    q.current = Some(job);
                    break job;
                }
                q = shared.wake.wait(q).unwrap();
            }
        };
        let msg = match render_page(&doc, job) {
            Some(image) => Message::Rendered(image),
            None => Message::Failed(job),
        };
        if tx.unbounded_send(msg).is_err() {
            return; // Fenster geschlossen
        }
    }
}

fn render_page(doc: &poppler::Document, job: Job) -> Option<Image> {
    let page = doc.page(i32::try_from(job.page).ok()?)?;
    let size = page.size();
    if size.0 <= 0.0 || size.1 <= 0.0 {
        return None;
    }
    let (w, h) = image_size(size, f64::from(job.key) / 1000.0);
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        // Papier weiß; PDFs ohne Hintergrund wären sonst durchsichtig.
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().ok()?;
        cr.scale(f64::from(w) / size.0, f64::from(h) / size.1);
        page.render(&cr);
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).ok()?;
    // Die Pixel wandern ohne Kopie zu GTK (die Bytes halten die Fläche).
    let data = surface.take_data().ok()?;
    Some(Image {
        job,
        width: w,
        height: h,
        stride,
        data: glib::Bytes::from_owned(data),
    })
}

fn search_loop(
    shared: &Shared,
    path: &std::path::Path,
    password: Option<&str>,
    tx: &UnboundedSender<Message>,
) {
    let Ok(doc) = document::open(path, password) else {
        return;
    };
    let n = usize::try_from(doc.n_pages()).unwrap_or(0);
    loop {
        let req = {
            let mut slot = shared.search.lock().unwrap();
            loop {
                if shared.stop.load(Ordering::SeqCst) {
                    return;
                }
                if let Some(req) = slot.take() {
                    break req;
                }
                slot = shared.search_wake.wait(slot).unwrap();
            }
        };
        let flags = if req.case_sensitive {
            poppler::FindFlags::CASE_SENSITIVE
        } else {
            poppler::FindFlags::DEFAULT
        };
        let start = if n == 0 { 0 } else { req.start % n };
        let mut cancelled = false;
        for page in (start..n).chain(0..start) {
            if shared.search_generation.load(Ordering::SeqCst) != req.generation {
                cancelled = true;
                break;
            }
            let rects = doc
                .page(page as i32)
                .map(|p| {
                    let h = p.size().1;
                    p.find_text_with_options(&req.query, flags)
                        .iter()
                        .map(|r| from_pdf_rect(r.x1(), r.y1(), r.x2(), r.y2(), h))
                        .collect()
                })
                .unwrap_or_default();
            let msg = Message::Found {
                generation: req.generation,
                page,
                rects,
            };
            if tx.unbounded_send(msg).is_err() {
                return;
            }
        }
        if !cancelled
            && tx
                .unbounded_send(Message::SearchDone {
                    generation: req.generation,
                })
                .is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::tests::{sample_pdf, temp};
    use std::time::{Duration, Instant};

    /// Wartet auf Nachrichten, bis `done` sie für ausreichend hält.
    fn collect(
        rx: &mut futures_channel::mpsc::UnboundedReceiver<Message>,
        mut done: impl FnMut(&Message) -> bool,
    ) -> Vec<Message> {
        let start = Instant::now();
        let mut out = Vec::new();
        while start.elapsed() < Duration::from_secs(20) {
            match rx.try_recv() {
                Ok(m) => {
                    let stop = done(&m);
                    out.push(m);
                    if stop {
                        return out;
                    }
                }
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        panic!("keine Antwort vom Hintergrundfaden");
    }

    #[test]
    fn pages_render_in_the_background() {
        let dir = temp("render");
        let (tx, mut rx) = futures_channel::mpsc::unbounded();
        let r = Renderer::start(sample_pdf(&dir), None, tx);
        let job = Job {
            page: 1,
            key: 1000,
            thumb: false,
        };
        r.set_jobs(vec![job]);
        let msgs = collect(&mut rx, |m| matches!(m, Message::Rendered(_)));
        let Some(Message::Rendered(img)) = msgs.last() else {
            panic!()
        };
        assert_eq!(img.job, job);
        // Querformat bei 1 px je pt
        assert_eq!((img.width, img.height), (842, 595));
        assert!(img.stride >= 842 * 4);
        assert_eq!(img.data.len(), img.stride * 595);
        // Papier weiß, aber nicht nur: Text ist schwarz gezeichnet
        assert!(img.data.chunks(4).any(|p| p[0] < 128));
        assert!(img.data.chunks(4).filter(|p| p[0] == 255).count() > 842 * 500);
        drop(r);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_reports_every_page_and_finishes() {
        let dir = temp("suche");
        let (tx, mut rx) = futures_channel::mpsc::unbounded();
        let r = Renderer::start(sample_pdf(&dir), None, tx);
        let generation = r.search("querencia", false, 1);
        let msgs = collect(&mut rx, |m| matches!(m, Message::SearchDone { .. }));
        let mut pages = Vec::new();
        for m in &msgs {
            if let Message::Found {
                generation: g,
                page,
                rects,
            } = m
            {
                assert_eq!(*g, generation);
                assert_eq!(rects.len(), 1, "Seite {page}");
                pages.push(*page);
                if *page == 1 {
                    // Treffer unten auf der Querseite: Ursprung oben links
                    assert!(rects[0].y > 400.0 && rects[0].y < 500.0, "{:?}", rects[0]);
                }
            }
        }
        // ab Seite 1, ringsum
        assert_eq!(pages, vec![1, 0]);
        // Groß-/Kleinschreibung beachten: kein Treffer
        let generation = r.search("querencia", true, 0);
        let msgs = collect(&mut rx, |m| matches!(m, Message::SearchDone { .. }));
        assert!(msgs.iter().all(|m| match m {
            Message::Found {
                generation: g,
                rects,
                ..
            } => *g != generation || rects.is_empty(),
            _ => true,
        }));
        drop(r);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn image_sizes_respect_the_limits() {
        // A4 bei 100 % (96 dpi)
        assert_eq!(image_size((595.0, 842.0), 96.0 / 72.0), (793, 1123));
        // A0-Poster bei 400 %: höchstens MAX_PIXELS, Seitenverhältnis bleibt
        let (w, h) = image_size((2384.0, 3370.0), 4.0 * 96.0 / 72.0);
        assert!(f64::from(w) * f64::from(h) <= MAX_PIXELS * 1.001);
        assert!((f64::from(w) / f64::from(h) - 2384.0 / 3370.0).abs() < 0.01);
        // sehr lange, schmale Seite: Kantenlänge begrenzt
        let (w, h) = image_size((100.0, 20000.0), 2.0);
        assert!(f64::from(h) <= MAX_EDGE && w >= 1);
        // winzige Seite: mindestens ein Pixel
        assert_eq!(image_size((0.1, 0.1), 0.1), (1, 1));
    }
}
