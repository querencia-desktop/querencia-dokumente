//! Zoom: feste Stufen wie Evince 2.30 (bis 400 %), dazu „Ganze Seite“
//! (Best Fit) und „Seitenbreite“ (Fit Page Width).
//!
//! 100 % heißt Originalgröße bei 96 dpi: ein PDF-Punkt (1/72 Zoll) wird zu
//! 96/72 Pixeln. Eine A4-Seite ist bei 100 % also 794 Pixel breit.

use crate::i18n::{self, gettext};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zoom {
    /// Ganze Seite sichtbar (Breite und Höhe passen ins Fenster).
    BestFit,
    /// Seitenbreite = Fensterbreite.
    FitWidth,
    /// Fester Faktor (1.0 = 100 %).
    Factor(f64),
}

/// Pixel je PDF-Punkt bei 100 %.
pub const PX_PER_PT: f64 = 96.0 / 72.0;
/// Kleinster und größter Faktor. Über 400 % würden einzelne Seiten mehr als
/// 60 MB Pixel brauchen (A4); Evince 2.30 endet ebenfalls bei 400 %.
pub const MIN: f64 = 0.05;
pub const MAX: f64 = 4.0;

/// Stufen für Vergrößern/Verkleinern (Strg+Plus/Minus, Strg+Mausrad).
pub const LEVELS: [f64; 13] = [
    0.1, 0.25, 0.333, 0.5, 0.7, 0.85, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0,
];
/// Stufen im Zoom-Menü der Werkzeugleiste (wie Evince 2.30).
pub const MENU_LEVELS: [f64; 10] = [0.5, 0.7, 0.85, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0];

/// Nächstgrößere Stufe vom aktuell sichtbaren Faktor aus.
pub fn zoom_in(current: f64) -> f64 {
    LEVELS
        .iter()
        .copied()
        .find(|&l| l > current * 1.001)
        .unwrap_or(MAX)
}

/// Nächstkleinere Stufe; unter der kleinsten Stufe bleibt der Faktor.
pub fn zoom_out(current: f64) -> f64 {
    LEVELS
        .iter()
        .rev()
        .copied()
        .find(|&l| l < current * 0.999)
        .unwrap_or_else(|| current.clamp(MIN, LEVELS[0]))
}

/// Faktor für „Seitenbreite“: die breiteste Seite (in Punkt) füllt
/// `avail` Pixel abzüglich der Ränder `border` links und rechts.
pub fn fit_width(page_w_pt: f64, avail: f64, border: f64) -> f64 {
    if page_w_pt <= 0.0 {
        return 1.0;
    }
    ((avail - 2.0 * border) / (page_w_pt * PX_PER_PT)).clamp(MIN, MAX)
}

/// Faktor für „Ganze Seite“: Seite `page` (Punkt) passt mit Rändern
/// vollständig in die Fläche `area` (Pixel).
pub fn best_fit(page: (f64, f64), area: (f64, f64), border: f64) -> f64 {
    let (pw, ph) = page;
    let (aw, ah) = area;
    if pw <= 0.0 || ph <= 0.0 {
        return 1.0;
    }
    let w = (aw - 2.0 * border) / (pw * PX_PER_PT);
    let h = (ah - 2.0 * border) / (ph * PX_PER_PT);
    w.min(h).clamp(MIN, MAX)
}

/// Zoomstufe für Statusleiste und Werkzeugleiste: „100 %“ im Deutschen,
/// „100%“ im Englischen (Form aus dem Katalog).
pub fn percent(f: f64) -> String {
    // TRANSLATORS: Zoomstufe; {value} ist eine ganze Zahl („125“)
    percent_with(f, &gettext("{value}%"))
}

fn percent_with(f: f64, template: &str) -> String {
    let p = (f * 100.0).round() as i64;
    i18n::fill(template, &[("value", &p.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_follow_the_levels_and_stay_within_bounds() {
        assert_eq!(zoom_in(1.0), 1.25);
        assert_eq!(zoom_out(1.0), 0.85);
        assert_eq!(zoom_in(1.1), 1.25);
        assert_eq!(zoom_out(1.1), 1.0);
        assert_eq!(zoom_in(3.5), 4.0);
        assert_eq!(zoom_in(4.0), MAX);
        assert_eq!(zoom_out(0.25), 0.1);
        // unter der kleinsten Stufe (z. B. nach „Ganze Seite“) bleibt es
        assert_eq!(zoom_out(0.08), 0.08);
        assert_eq!(zoom_out(0.01), MIN);
        // Rundungsreste: knapp unter 100 % gilt als 100 %
        assert_eq!(zoom_in(0.9999999), 1.25);
        assert_eq!(zoom_out(1.0000001), 0.85);
    }

    #[test]
    fn levels_are_sorted_and_cover_the_menu() {
        assert!(LEVELS.windows(2).all(|w| w[0] < w[1]));
        assert!(MENU_LEVELS.iter().all(|l| LEVELS.contains(l)));
        assert_eq!(*LEVELS.last().unwrap(), MAX);
    }

    #[test]
    fn fit_width_fills_the_area_minus_borders() {
        // A4 = 595 × 842 pt; 100 % = 793,3 px
        let f = fit_width(595.0, 793.333 + 24.0, 12.0);
        assert!((f - 1.0).abs() < 1e-3, "{f}");
        let f = fit_width(595.0, 400.0 + 24.0, 12.0);
        assert!((f * 595.0 * PX_PER_PT - 400.0).abs() < 1e-6);
        assert_eq!(fit_width(0.0, 500.0, 12.0), 1.0);
        assert_eq!(fit_width(595.0, 100_000.0, 12.0), MAX);
    }

    #[test]
    fn best_fit_takes_the_tighter_side() {
        // hohes Fenster: Breite begrenzt
        let f = best_fit((595.0, 842.0), (624.0, 4000.0), 12.0);
        assert!((f * 595.0 * PX_PER_PT - 600.0).abs() < 1e-6);
        // breites Fenster: Höhe begrenzt
        let f = best_fit((595.0, 842.0), (4000.0, 624.0), 12.0);
        assert!((f * 842.0 * PX_PER_PT - 600.0).abs() < 1e-6);
        // Querformat
        let f = best_fit((842.0, 595.0), (1000.0, 1000.0), 0.0);
        assert!((f * 842.0 * PX_PER_PT - 1000.0).abs() < 1e-6);
        assert_eq!(best_fit((0.0, 10.0), (100.0, 100.0), 0.0), 1.0);
    }

    #[test]
    fn percentages_follow_the_catalog() {
        assert_eq!(percent_with(1.0, "{value} %"), "100 %");
        assert_eq!(percent_with(0.7, "{value} %"), "70 %");
        assert_eq!(percent_with(0.333, "{value} %"), "33 %");
        // ohne Katalog: Englisch
        assert_eq!(percent(1.25), "125%");
    }
}
