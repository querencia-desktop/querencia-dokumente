//! Textsuche: Treffer je Seite und der aktuelle Treffer für Weiter/Zurück.
//!
//! Gesucht wird in einem eigenen Faden (render.rs); die Treffer kommen Seite
//! für Seite und in beliebiger Reihenfolge an (die Suche beginnt bei der
//! aktuellen Seite und läuft ringsum). Dieses Modell ordnet sie nach Seite
//! und Lage.

use std::collections::BTreeMap;

use crate::layout::Rect;

/// Poppler liefert Suchtreffer mit Ursprung **unten** links (PDF-Koordinaten);
/// die Ansicht rechnet mit Ursprung oben links.
pub fn from_pdf_rect(x1: f64, y1: f64, x2: f64, y2: f64, page_h: f64) -> Rect {
    let (left, right) = (x1.min(x2), x1.max(x2));
    let (low, high) = (y1.min(y2), y1.max(y2));
    Rect::new(left, page_h - high, right - left, high - low)
}

#[derive(Debug, Default)]
pub struct Search {
    pub query: String,
    pub case_sensitive: bool,
    pages: BTreeMap<usize, Vec<Rect>>,
    searched: usize,
    total_pages: usize,
    count: usize,
    current: Option<(usize, usize)>,
    done: bool,
}

impl Search {
    pub fn new(query: &str, case_sensitive: bool, total_pages: usize) -> Self {
        Self {
            query: query.to_string(),
            case_sensitive,
            total_pages,
            ..Default::default()
        }
    }

    /// Ergebnis einer Seite (auch ohne Treffer: zählt als durchsucht).
    /// Treffer einer Seite werden von oben nach unten, links nach rechts geordnet.
    pub fn add_page(&mut self, page: usize, mut rects: Vec<Rect>) {
        self.searched += 1;
        if rects.is_empty() {
            return;
        }
        rects.sort_by(|a, b| {
            (a.y, a.x)
                .partial_cmp(&(b.y, b.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.count += rects.len();
        if let Some(old) = self.pages.insert(page, rects) {
            self.count -= old.len();
        }
    }

    pub fn finish(&mut self) {
        self.done = true;
    }

    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Anzahl der durchsuchten Seiten (Fortschritt).
    pub fn searched(&self) -> usize {
        self.searched
    }

    pub fn total_pages(&self) -> usize {
        self.total_pages
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn current(&self) -> Option<(usize, usize)> {
        self.current
    }

    pub fn current_rect(&self) -> Option<(usize, Rect)> {
        let (p, i) = self.current?;
        self.pages.get(&p).and_then(|r| r.get(i)).map(|r| (p, *r))
    }

    /// Laufende Nummer des aktuellen Treffers (1-basiert) in Seitenfolge.
    pub fn current_number(&self) -> Option<usize> {
        let (p, i) = self.current?;
        let before: usize = self.pages.range(..p).map(|(_, r)| r.len()).sum();
        Some(before + i + 1)
    }

    pub fn matches_on(&self, page: usize) -> &[Rect] {
        self.pages.get(&page).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Erster Treffer ab Seite `from` (ringsum), wenn es noch keinen aktuellen
    /// gibt; sonst der nächste nach dem aktuellen. Setzt und liefert ihn.
    pub fn next(&mut self, from: usize) -> Option<(usize, usize)> {
        let found = match self.current {
            None => self.first_at_or_after(from),
            Some((p, i)) => {
                if i + 1 < self.matches_on(p).len() {
                    Some((p, i + 1))
                } else {
                    self.first_at_or_after(p + 1)
                }
            }
        };
        if found.is_some() {
            self.current = found;
        }
        found
    }

    /// Wie [`Self::next`], rückwärts.
    pub fn prev(&mut self, from: usize) -> Option<(usize, usize)> {
        let found = match self.current {
            None => self.last_at_or_before(from),
            Some((p, i)) if i > 0 => Some((p, i - 1)),
            Some((p, _)) => match p.checked_sub(1) {
                Some(before) => self.last_at_or_before(before),
                None => self.last_at_or_before(usize::MAX),
            },
        };
        if found.is_some() {
            self.current = found;
        }
        found
    }

    fn first_at_or_after(&self, from: usize) -> Option<(usize, usize)> {
        self.pages
            .range(from..)
            .next()
            .or_else(|| self.pages.iter().next())
            .map(|(p, _)| (*p, 0))
    }

    fn last_at_or_before(&self, from: usize) -> Option<(usize, usize)> {
        self.pages
            .range(..=from)
            .next_back()
            .or_else(|| self.pages.iter().next_back())
            .map(|(p, r)| (*p, r.len() - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(y: f64) -> Rect {
        Rect::new(10.0, y, 20.0, 10.0)
    }

    fn sample() -> Search {
        let mut s = Search::new("Querencia", false, 10);
        // Reihenfolge wie aus dem Faden: ab Seite 4, ringsum
        s.add_page(4, vec![r(300.0), r(100.0)]);
        s.add_page(5, vec![]);
        s.add_page(8, vec![r(50.0)]);
        s.add_page(1, vec![r(700.0)]);
        s.finish();
        s
    }

    #[test]
    fn pdf_rects_are_flipped_to_a_top_left_origin() {
        // Treffer 10 pt über dem unteren Rand einer 842 pt hohen Seite
        let x = from_pdf_rect(100.0, 10.0, 150.0, 22.0, 842.0);
        assert_eq!(x, Rect::new(100.0, 820.0, 50.0, 12.0));
        // vertauschte Ecken
        assert_eq!(from_pdf_rect(150.0, 22.0, 100.0, 10.0, 842.0), x);
    }

    #[test]
    fn matches_are_counted_and_ordered() {
        let s = sample();
        assert_eq!(s.count(), 4);
        assert_eq!(s.searched(), 4);
        assert!(s.is_done());
        assert_eq!(s.matches_on(4)[0].y, 100.0);
        assert_eq!(s.matches_on(5).len(), 0);
        assert_eq!(s.current(), None);
        assert_eq!(s.current_number(), None);
    }

    #[test]
    fn next_starts_at_the_view_page_and_wraps() {
        let mut s = sample();
        assert_eq!(s.next(3), Some((4, 0)));
        assert_eq!(s.current_number(), Some(2));
        assert_eq!(s.next(3), Some((4, 1)));
        assert_eq!(s.next(3), Some((8, 0)));
        assert_eq!(s.current_number(), Some(4));
        // ringsum zum Anfang
        assert_eq!(s.next(3), Some((1, 0)));
        assert_eq!(s.current_number(), Some(1));
        assert_eq!(s.current_rect(), Some((1, r(700.0))));
        // ab einer Seite hinter dem letzten Treffer
        let mut t = sample();
        assert_eq!(t.next(9), Some((1, 0)));
    }

    #[test]
    fn prev_runs_backwards_and_wraps() {
        let mut s = sample();
        assert_eq!(s.prev(6), Some((4, 1)));
        assert_eq!(s.prev(6), Some((4, 0)));
        assert_eq!(s.prev(6), Some((1, 0)));
        assert_eq!(s.prev(6), Some((8, 0)));
        let mut t = sample();
        assert_eq!(t.prev(0), Some((8, 0)));
    }

    #[test]
    fn nothing_found_keeps_no_current_match() {
        let mut s = Search::new("fehlt", true, 3);
        s.add_page(0, vec![]);
        assert_eq!(s.next(0), None);
        assert_eq!(s.prev(0), None);
        assert_eq!(s.count(), 0);
        assert!(s.case_sensitive);
    }

    #[test]
    fn a_page_reported_twice_is_not_counted_twice() {
        let mut s = Search::new("x", false, 3);
        s.add_page(1, vec![r(1.0), r(2.0)]);
        s.add_page(1, vec![r(1.0)]);
        assert_eq!(s.count(), 1);
    }
}
