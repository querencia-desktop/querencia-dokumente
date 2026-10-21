//! Prüfprogramm (0.1.3): rendert jede Seite eines PDFs wie die Hauptansicht
//! (render.rs: weißes Papier, `cr.scale`, `page.render`, `take_data`) in den
//! angegebenen Maßstäben und meldet je Seite den Anteil nicht-weißer Pixel
//! und, falls ein Schritt scheitert, welcher.
//!
//! Aufruf: cargo run --example seitenpruefung -- <pdf> [maßstab …]
//!         … <pdf> --wirbel N [maßstab …]          zufällige Folge
//!         … <pdf> --umschreiben <neu.pdf> [maßstab …]  Datei ändert sich
//! Maßstab in Pixel je Punkt; ohne Angabe 0.25, 0.667, 1.333, 2.0
//! (Miniatur, 50 %, 100 %, 150 % bei 96 dpi).

use std::path::Path;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // --wirbel N: N Aufträge in zufälliger Folge (Seite, Maßstab) wie der
    // Renderfaden sie bekommt; dazu ein Suchfaden mit eigenem Dokument.
    let wirbel = args.iter().position(|a| a == "--wirbel").map(|i| {
        let n = args.get(i + 1).and_then(|n| n.parse().ok()).unwrap_or(1000);
        args.drain(i..(i + 2).min(args.len()));
        n
    });
    let umschreiben = args.iter().position(|a| a == "--umschreiben").map(|i| {
        let neu = args.get(i + 1).cloned().unwrap_or_default();
        args.drain(i..(i + 2).min(args.len()));
        neu
    });
    let mut args = args.into_iter();
    let Some(pdf) = args.next() else {
        eprintln!("Aufruf: seitenpruefung <pdf> [--wirbel N | --umschreiben neu.pdf] [maßstab …]");
        std::process::exit(2);
    };
    let mut scales: Vec<f64> = args.filter_map(|a| a.parse().ok()).collect();
    if scales.is_empty() {
        scales = vec![0.25, 0.667, 1.333, 2.0];
    }
    let path = std::fs::canonicalize(Path::new(&pdf)).expect("PDF");
    let uri = format!("file://{}", path.display());
    let doc = poppler::Document::from_file(&uri, None).expect("PDF öffnen");
    // --umschreiben <neu.pdf>: wie im Befund 21.10.: Kopie öffnen, Miniaturen
    // rendern, Kopie am selben Inode mit der neuen Fassung überschreiben
    // (wie WeasyPrint), dann jede Seite in Hauptansichtsgröße rendern.
    if let Some(neu) = umschreiben {
        std::process::exit(umschreiben_pruefen(&path, Path::new(&neu), &scales));
    }
    if let Some(n) = wirbel {
        std::process::exit(wirbeln(&doc, &uri, &scales, n));
    }
    let mut leer = 0;
    print!("seite");
    for s in &scales {
        print!("\t{s}");
    }
    println!();
    for i in 0..doc.n_pages() {
        let page = doc.page(i).expect("Seite");
        print!("{}", i + 1);
        for &s in &scales {
            match render(&page, s) {
                Ok(anteil) => {
                    if anteil < 0.001 {
                        leer += 1;
                    }
                    print!("\t{anteil:.4}");
                }
                Err(e) => {
                    leer += 1;
                    print!("\tFEHLER:{e}");
                }
            }
        }
        println!();
    }
    println!("leer oder Fehler: {leer}");
    std::process::exit(i32::from(leer > 0));
}

/// Anteil nicht-weißer Pixel; Fehler nennt den gescheiterten Schritt.
fn render(page: &poppler::Page, scale: f64) -> Result<f64, String> {
    let size = page.size();
    let (w, h) = (
        (size.0 * scale).round().max(1.0) as i32,
        (size.1 * scale).round().max(1.0) as i32,
    );
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
        .map_err(|e| format!("surface:{e}"))?;
    {
        let cr = cairo::Context::new(&surface).map_err(|e| format!("context:{e}"))?;
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().map_err(|e| format!("paint:{e}"))?;
        cr.scale(f64::from(w) / size.0, f64::from(h) / size.1);
        page.render(&cr);
        cr.status().map_err(|e| format!("render:{e}"))?;
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.take_data().map_err(|e| format!("take_data:{e}"))?;
    let mut bunt = 0usize;
    for row in data.chunks(stride).take(h as usize) {
        for p in row[..w as usize * 4].chunks(4) {
            if p[0] < 250 || p[1] < 250 || p[2] < 250 {
                bunt += 1;
            }
        }
    }
    Ok(bunt as f64 / (f64::from(w) * f64::from(h)))
}

/// Zufällige Folge von Aufträgen; meldet jede leere oder gescheiterte Seite.
fn wirbeln(doc: &poppler::Document, uri: &str, scales: &[f64], n: usize) -> i32 {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let suche = {
        let (stop, uri) = (stop.clone(), uri.to_string());
        std::thread::spawn(move || {
            let doc = poppler::Document::from_file(&uri, None).expect("PDF öffnen");
            let mut i = 0;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(p) = doc.page(i % doc.n_pages()) {
                    p.find_text_with_options("Querencia", poppler::FindFlags::DEFAULT);
                }
                i += 1;
            }
        })
    };
    let mut zustand: u64 = 0x2545_f491_4f6c_dd1d;
    let mut zufall = |m: usize| {
        zustand ^= zustand << 13;
        zustand ^= zustand >> 7;
        zustand ^= zustand << 17;
        (zustand % m as u64) as usize
    };
    let pages = doc.n_pages() as usize;
    let mut schlecht = 0;
    for k in 0..n {
        let (p, s) = (zufall(pages), scales[zufall(scales.len())]);
        let page = doc.page(p as i32).expect("Seite");
        match render(&page, s) {
            Ok(a) if a >= 0.001 => {}
            Ok(a) => {
                schlecht += 1;
                println!("Auftrag {k}: Seite {} bei {s} leer ({a:.5})", p + 1);
            }
            Err(e) => {
                schlecht += 1;
                println!("Auftrag {k}: Seite {} bei {s} FEHLER {e}", p + 1);
            }
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    suche.join().ok();
    println!("{n} Aufträge, leer oder Fehler: {schlecht}");
    i32::from(schlecht > 0)
}

fn umschreiben_pruefen(alt: &Path, neu: &Path, scales: &[f64]) -> i32 {
    use std::io::Write;
    let ordner = std::env::temp_dir().join(format!("seitenpruefung-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let kopie = ordner.join("dokument.pdf");
    std::fs::copy(alt, &kopie).expect("Kopie");
    let doc = poppler::Document::from_file(&format!("file://{}", kopie.display()), None)
        .expect("PDF öffnen");
    let n = doc.n_pages();
    for i in 0..n {
        render(&doc.page(i).expect("Seite"), 110.0 / 595.276).ok();
    }
    // Wie write_pdf(): öffnen mit Kürzen, neu schreiben – derselbe Inode
    let inhalt = std::fs::read(neu).expect("neue Fassung");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&kopie)
        .expect("überschreiben");
    f.write_all(&inhalt).expect("schreiben");
    drop(f);
    // 0.1.2: weiter aus dem offenen Dokument; 0.1.3: Stempel geändert →
    // neu öffnen (wie viewer.rs nach Message::Changed)
    let alt_leer = alle(&doc, scales, "ohne Neuladen (0.1.2)");
    let doc = poppler::Document::from_file(&format!("file://{}", kopie.display()), None)
        .expect("PDF neu öffnen");
    let neu_leer = alle(&doc, scales, "mit Neuladen (0.1.3)");
    std::fs::remove_dir_all(&ordner).ok();
    let gesamt = doc.n_pages() as usize * scales.len();
    println!(
        "nach dem Überschreiben leer oder Fehler: ohne Neuladen {alt_leer} von {gesamt}, mit Neuladen {neu_leer} von {gesamt}"
    );
    i32::from(neu_leer > 0)
}

fn alle(doc: &poppler::Document, scales: &[f64], art: &str) -> usize {
    let mut leer = 0;
    for i in 0..doc.n_pages() {
        let page = doc.page(i).expect("Seite");
        for &s in scales {
            match render(&page, s) {
                Ok(a) if a >= 0.001 => println!("{art}: Seite {} bei {s}: {a:.4}", i + 1),
                Ok(a) => {
                    leer += 1;
                    println!("{art}: Seite {} bei {s}: LEER {a:.5}", i + 1);
                }
                Err(e) => {
                    leer += 1;
                    println!("{art}: Seite {} bei {s}: FEHLER {e}", i + 1);
                }
            }
        }
    }
    leer
}
