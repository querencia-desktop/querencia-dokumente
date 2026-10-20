//! Seitenmodell der fortlaufenden Ansicht: Seiten untereinander, waagrecht
//! zentriert, mit festem Rand und Abstand. Alle Werte in Pixeln der Ansicht;
//! die Seitengrößen kommen in PDF-Punkt und werden mit `scale` (Pixel je
//! Punkt) umgerechnet.

use std::ops::Range;

/// Rand um die Seitenfolge (Pixel).
pub const BORDER: f64 = 12.0;
/// Abstand zwischen zwei Seiten (Pixel).
pub const GAP: f64 = 12.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Stelle im Dokument, die beim Zoomen an derselben Bildschirmstelle bleiben
/// soll: Seite und Anteil der Seitenhöhe bzw. -breite (außerhalb von 0..1 im
/// Rand oder Zwischenraum).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub page: usize,
    pub fx: f64,
    pub fy: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// Seitengrößen in Punkt.
    sizes: Vec<(f64, f64)>,
    /// Pixel je Punkt.
    scale: f64,
    /// Oberkante jeder Seite.
    tops: Vec<f64>,
    max_w: f64,
    height: f64,
}

impl Layout {
    pub fn new(sizes: Vec<(f64, f64)>, scale: f64) -> Self {
        let mut l = Self {
            sizes,
            scale,
            ..Default::default()
        };
        l.relayout();
        l
    }

    fn relayout(&mut self) {
        self.tops.clear();
        let mut y = BORDER;
        let mut max_w: f64 = 0.0;
        for &(w, h) in &self.sizes {
            self.tops.push(y);
            y += h * self.scale + GAP;
            max_w = max_w.max(w * self.scale);
        }
        self.max_w = max_w;
        self.height = if self.sizes.is_empty() {
            0.0
        } else {
            y - GAP + BORDER
        };
    }

    pub fn set_scale(&mut self, scale: f64) {
        if (scale - self.scale).abs() > f64::EPSILON {
            self.scale = scale;
            self.relayout();
        }
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn len(&self) -> usize {
        self.sizes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }

    pub fn page_size_pt(&self, i: usize) -> (f64, f64) {
        self.sizes.get(i).copied().unwrap_or((0.0, 0.0))
    }

    /// Größte Seitenbreite und -höhe in Punkt (Grundlage der Fit-Modi).
    pub fn max_size_pt(&self) -> (f64, f64) {
        self.sizes.iter().fold((0.0, 0.0), |(mw, mh), &(w, h)| {
            (f64::max(mw, w), f64::max(mh, h))
        })
    }

    pub fn content_width(&self) -> f64 {
        if self.sizes.is_empty() {
            0.0
        } else {
            self.max_w + 2.0 * BORDER
        }
    }

    pub fn content_height(&self) -> f64 {
        self.height
    }

    /// Seite `i` als Rechteck; `area_w` ist die sichtbare Breite (Seiten
    /// werden in der breiteren von Inhalt und Fläche zentriert).
    pub fn page_rect(&self, i: usize, area_w: f64) -> Rect {
        let (w, h) = self.page_size_pt(i);
        let (w, h) = (w * self.scale, h * self.scale);
        let total = self.content_width().max(area_w);
        let x = ((total - w) / 2.0).floor();
        Rect::new(x, self.tops.get(i).copied().unwrap_or(0.0), w, h)
    }

    /// Seiten, die den senkrechten Bereich `y0..y1` berühren.
    pub fn visible(&self, y0: f64, y1: f64) -> Range<usize> {
        let n = self.sizes.len();
        // erste Seite, deren Unterkante unter y0 liegt
        let first = self
            .tops
            .partition_point(|&t| t < y0)
            .saturating_sub(1)
            .min(n);
        let mut first = first;
        while first < n && self.bottom(first) < y0 {
            first += 1;
        }
        let end = self.tops.partition_point(|&t| t < y1).max(first);
        first..end
    }

    fn bottom(&self, i: usize) -> f64 {
        self.tops[i] + self.sizes[i].1 * self.scale
    }

    /// Aktuelle Seite für die Anzeige „Seite X von Y“: die erste Seite, von
    /// der mindestens die Hälfte (ihrer selbst oder des Fensters) sichtbar ist;
    /// sonst die Seite mit dem größten sichtbaren Anteil.
    pub fn current_page(&self, y0: f64, view_h: f64) -> usize {
        let y1 = y0 + view_h;
        let mut best = (0, -1.0);
        for i in self.visible(y0, y1) {
            let top = self.tops[i];
            let bottom = self.bottom(i);
            let shown = (bottom.min(y1) - top.max(y0)).max(0.0);
            let h = bottom - top;
            if shown >= 0.5 * h.min(view_h) - 0.5 {
                return i;
            }
            if shown > best.1 {
                best = (i, shown);
            }
        }
        best.0
    }

    /// Bildlaufwert, bei dem Seite `i` oben im Fenster steht (mit halbem
    /// Abstand darüber).
    pub fn scroll_target(&self, i: usize) -> f64 {
        self.tops
            .get(i)
            .map(|t| (t - GAP / 2.0).max(0.0))
            .unwrap_or(0.0)
    }

    /// Dokumentstelle zum Ansichtspunkt (x, y).
    pub fn anchor_at(&self, x: f64, y: f64, area_w: f64) -> Option<Anchor> {
        if self.sizes.is_empty() {
            return None;
        }
        // Seite, in deren Bereich (inklusive halbem Abstand) y liegt
        let page = self
            .tops
            .partition_point(|&t| t <= y + GAP / 2.0)
            .saturating_sub(1);
        let r = self.page_rect(page, area_w);
        Some(Anchor {
            page,
            fx: if r.w > 0.0 { (x - r.x) / r.w } else { 0.0 },
            fy: if r.h > 0.0 { (y - r.y) / r.h } else { 0.0 },
        })
    }

    /// Ansichtspunkt einer Dokumentstelle (Umkehrung von [`Self::anchor_at`]).
    pub fn point_of(&self, a: Anchor, area_w: f64) -> (f64, f64) {
        let r = self.page_rect(a.page.min(self.len().saturating_sub(1)), area_w);
        (r.x + a.fx * r.w, r.y + a.fy * r.h)
    }

    /// Seite und Punkt (in PDF-Punkt, Ursprung oben links) unter (x, y).
    /// `clamp`: außerhalb der Seiten auf die nächste Seite begrenzen
    /// (Textauswahl mit gezogener Maus).
    pub fn hit(&self, x: f64, y: f64, area_w: f64, clamp: bool) -> Option<(usize, f64, f64)> {
        let a = self.anchor_at(x, y, area_w)?;
        let inside = (0.0..=1.0).contains(&a.fx) && (0.0..=1.0).contains(&a.fy);
        if !inside && !clamp {
            return None;
        }
        let (w, h) = self.page_size_pt(a.page);
        Some((a.page, a.fx.clamp(0.0, 1.0) * w, a.fy.clamp(0.0, 1.0) * h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f64, f64) = (595.0, 842.0);

    fn three() -> Layout {
        // Hochformat, Querformat, Hochformat; 1 px je pt
        Layout::new(vec![A4, (842.0, 595.0), A4], 1.0)
    }

    #[test]
    fn pages_stack_with_border_and_gap() {
        let l = three();
        assert_eq!(l.len(), 3);
        assert_eq!(l.page_rect(0, 0.0).y, BORDER);
        assert_eq!(l.page_rect(1, 0.0).y, BORDER + 842.0 + GAP);
        assert_eq!(l.page_rect(2, 0.0).y, BORDER + 842.0 + GAP + 595.0 + GAP);
        assert_eq!(
            l.content_height(),
            BORDER + 842.0 + GAP + 595.0 + GAP + 842.0 + BORDER
        );
        assert_eq!(l.content_width(), 842.0 + 2.0 * BORDER);
        assert_eq!(l.max_size_pt(), (842.0, 842.0));
    }

    #[test]
    fn pages_are_centred_in_the_wider_of_content_and_area() {
        let l = three();
        // Fläche schmaler als der Inhalt: in der Inhaltsbreite zentriert
        let r = l.page_rect(0, 100.0);
        assert_eq!(r.x, ((866.0 - 595.0) / 2.0_f64).floor());
        assert_eq!(l.page_rect(1, 100.0).x, BORDER);
        // breite Fläche
        assert_eq!(
            l.page_rect(0, 2000.0).x,
            ((2000.0 - 595.0) / 2.0_f64).floor()
        );
    }

    #[test]
    fn scale_changes_relayout() {
        let mut l = three();
        l.set_scale(0.5);
        assert_eq!(l.page_rect(1, 0.0).y, BORDER + 421.0 + GAP);
        assert_eq!(l.page_rect(1, 0.0).w, 421.0);
        assert_eq!(l.scale(), 0.5);
    }

    #[test]
    fn visible_range_and_current_page() {
        let l = three();
        let p1 = l.page_rect(1, 0.0);
        assert_eq!(l.visible(0.0, 100.0), 0..1);
        assert_eq!(l.visible(0.0, p1.y + 1.0), 0..2);
        // nur der Zwischenraum sichtbar
        assert_eq!(l.visible(p1.y - GAP + 1.0, p1.y - 1.0), 1..1);
        assert_eq!(l.visible(0.0, 1e9), 0..3);
        assert_eq!(l.visible(1e9, 2e9), 3..3);
        // aktuelle Seite
        assert_eq!(l.current_page(0.0, 600.0), 0);
        assert_eq!(l.current_page(l.scroll_target(1), 600.0), 1);
        assert_eq!(l.current_page(l.scroll_target(2), 600.0), 2);
        // Seite 0 nur noch zu einem Viertel zu sehen: Seite 1 ist aktuell
        assert_eq!(l.current_page(p1.y - 150.0, 600.0), 1);
        // leeres Dokument
        assert_eq!(Layout::new(vec![], 1.0).visible(0.0, 100.0), 0..0);
        assert_eq!(Layout::new(vec![], 1.0).current_page(0.0, 100.0), 0);
    }

    #[test]
    fn many_small_pages_report_the_first_mostly_visible() {
        // 500 Seiten bei 10 %: viele Seiten gleichzeitig sichtbar
        let mut l = Layout::new(vec![A4; 500], 1.0);
        l.set_scale(0.1);
        let y = l.scroll_target(250);
        assert_eq!(l.current_page(y, 768.0), 250);
        assert_eq!(l.visible(y, y + 768.0).start, 250);
        assert!(l.visible(y, y + 768.0).len() > 5);
    }

    #[test]
    fn anchors_survive_a_zoom() {
        let mut l = three();
        let p = l.page_rect(1, 1000.0);
        let a = l
            .anchor_at(p.x + p.w / 4.0, p.y + p.h / 2.0, 1000.0)
            .unwrap();
        assert_eq!(a.page, 1);
        assert!((a.fx - 0.25).abs() < 1e-9 && (a.fy - 0.5).abs() < 1e-9);
        l.set_scale(2.0);
        let (x, y) = l.point_of(a, 1000.0);
        let q = l.page_rect(1, 1000.0);
        assert!((x - (q.x + q.w / 4.0)).abs() < 1e-9);
        assert!((y - (q.y + q.h / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn hits_are_in_page_points_with_top_left_origin() {
        let l = Layout::new(vec![A4, A4], 2.0);
        let r = l.page_rect(1, 0.0);
        let (page, x, y) = l.hit(r.x + 20.0, r.y + 10.0, 0.0, false).unwrap();
        assert_eq!(page, 1);
        assert!((x - 10.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9);
        // Rand: ohne clamp kein Treffer, mit clamp auf die Seite begrenzt
        assert!(l.hit(1.0, 1.0, 0.0, false).is_none());
        let (page, x, y) = l.hit(1.0, 1.0, 0.0, true).unwrap();
        assert_eq!((page, x, y), (0, 0.0, 0.0));
        // Zwischenraum unterhalb von Seite 0, nah an Seite 1: Seite 1
        let (page, _, y) = l.hit(r.x + 5.0, r.y - 2.0, 0.0, true).unwrap();
        assert_eq!((page, y), (1, 0.0));
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(0.0, 0.0) && r.contains(9.9, 9.9));
        assert!(!r.contains(10.0, 5.0) && !r.contains(-0.1, 5.0));
    }
}
