//! Textauswahl über Seitengrenzen hinweg: Anfang und Ende als (Seite, Punkt),
//! je Seite der Bereich, den Poppler auswählen soll (Ursprung oben links,
//! PDF-Punkt).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub page: usize,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub start: Point,
    pub end: Point,
}

impl Selection {
    /// Ganzes Dokument („Alles auswählen“); `last` = (letzte Seite, Breite, Höhe).
    pub fn all(last: (usize, f64, f64)) -> Self {
        Self {
            start: Point {
                page: 0,
                x: 0.0,
                y: 0.0,
            },
            end: Point {
                page: last.0,
                x: last.1,
                y: last.2,
            },
        }
    }

    /// Anfang und Ende in Leserichtung (frühere Seite zuerst). Auf derselben
    /// Seite bleibt die Zugrichtung erhalten; Poppler ordnet selbst.
    pub fn ordered(&self) -> (Point, Point) {
        if self.end.page < self.start.page {
            (self.end, self.start)
        } else {
            (self.start, self.end)
        }
    }

    pub fn pages(&self) -> std::ops::RangeInclusive<usize> {
        let (a, b) = self.ordered();
        a.page..=b.page
    }

    /// Ausgewählter Bereich auf `page` als (x1, y1, x2, y2); `size` ist die
    /// Seitengröße in Punkt. Erste Seite: vom Anfang bis Seitenende, mittlere
    /// Seiten ganz, letzte Seite vom Seitenanfang bis zum Ende.
    pub fn area_on(&self, page: usize, size: (f64, f64)) -> Option<(f64, f64, f64, f64)> {
        let (a, b) = self.ordered();
        if page < a.page || page > b.page {
            return None;
        }
        let (x1, y1) = if page == a.page {
            (a.x, a.y)
        } else {
            (0.0, 0.0)
        };
        let (x2, y2) = if page == b.page {
            (b.x, b.y)
        } else {
            (size.0, size.1)
        };
        Some((x1, y1, x2, y2))
    }

    /// Kein Bereich (Klick ohne Ziehen).
    pub fn is_empty(&self) -> bool {
        self.start.page == self.end.page
            && (self.start.x - self.end.x).abs() < 0.5
            && (self.start.y - self.end.y).abs() < 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(page: usize, x: f64, y: f64) -> Point {
        Point { page, x, y }
    }

    const A4: (f64, f64) = (595.0, 842.0);

    #[test]
    fn one_page_keeps_both_points() {
        let s = Selection {
            start: p(2, 50.0, 400.0),
            end: p(2, 10.0, 100.0),
        };
        assert_eq!(s.area_on(2, A4), Some((50.0, 400.0, 10.0, 100.0)));
        assert_eq!(s.area_on(1, A4), None);
        assert_eq!(s.pages(), 2..=2);
        assert!(!s.is_empty());
    }

    #[test]
    fn several_pages_split_into_head_body_and_tail() {
        // nach oben gezogen: Ende liegt auf einer früheren Seite
        let s = Selection {
            start: p(5, 300.0, 200.0),
            end: p(3, 100.0, 700.0),
        };
        assert_eq!(s.pages(), 3..=5);
        assert_eq!(s.area_on(3, A4), Some((100.0, 700.0, 595.0, 842.0)));
        assert_eq!(s.area_on(4, A4), Some((0.0, 0.0, 595.0, 842.0)));
        assert_eq!(s.area_on(5, A4), Some((0.0, 0.0, 300.0, 200.0)));
        assert_eq!(s.area_on(6, A4), None);
    }

    #[test]
    fn select_all_spans_the_document() {
        let s = Selection::all((9, 595.0, 842.0));
        assert_eq!(s.pages(), 0..=9);
        assert_eq!(s.area_on(0, A4), Some((0.0, 0.0, 595.0, 842.0)));
        assert_eq!(s.area_on(9, A4), Some((0.0, 0.0, 595.0, 842.0)));
    }

    #[test]
    fn a_click_is_an_empty_selection() {
        let s = Selection {
            start: p(0, 10.0, 10.0),
            end: p(0, 10.2, 10.1),
        };
        assert!(s.is_empty());
    }
}
