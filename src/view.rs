//! Die Dokumentansicht: fortlaufende Seiten in einem eigenen scrollbaren
//! Widget (`gtk::Scrollable`). Gezeichnet werden nur die sichtbaren Seiten,
//! als Texturen aus dem Seiten-Cache; fehlt eine Seite im passenden Maßstab,
//! steht bis zum Eintreffen des neuen Bildes das nächstbeste (oder weißes
//! Papier) da. Das Rendern selbst macht der Hintergrundfaden (render.rs).
//!
//! Dazu Textauswahl mit der Maus (Poppler-Glyphenauswahl) und die
//! Hervorhebung der Suchtreffer. Farben der Auswahl und Treffer kommen aus dem
//! GTK-Thema (`theme_selected_bg_color`), nicht aus eigener CSS.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::cache::{Cache, scale_key};
use crate::document::Doc;
use crate::layout::{Anchor, BORDER, Layout, Rect};
use crate::search::Search;
use crate::selection::{Point, Selection};
use crate::zoom::{self, PX_PER_PT, Zoom};

/// Speicherobergrenze des Seiten-Caches; `QUERENCIA_DOKUMENTE_CACHE_MB`
/// ändert sie (Messungen).
pub fn cache_limit() -> usize {
    std::env::var("QUERENCIA_DOKUMENTE_CACHE_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&mb| mb >= 8)
        .unwrap_or(128)
        * 1024
        * 1024
}

/// Bildlauf je Pfeiltaste (Pixel).
const STEP: f64 = 48.0;

type Callback = Rc<dyn Fn()>;

mod imp {
    use super::*;

    #[derive(glib::Properties)]
    #[properties(wrapper_type = super::DocView)]
    pub struct DocView {
        #[property(get, set = Self::set_hadjustment, override_interface = gtk::Scrollable, nullable)]
        pub hadjustment: RefCell<Option<gtk::Adjustment>>,
        #[property(get, set = Self::set_vadjustment, override_interface = gtk::Scrollable, nullable)]
        pub vadjustment: RefCell<Option<gtk::Adjustment>>,
        #[property(get, set, override_interface = gtk::Scrollable, builder(gtk::ScrollablePolicy::Minimum))]
        pub hscroll_policy: Cell<gtk::ScrollablePolicy>,
        #[property(get, set, override_interface = gtk::Scrollable, builder(gtk::ScrollablePolicy::Minimum))]
        pub vscroll_policy: Cell<gtk::ScrollablePolicy>,

        pub doc: RefCell<Option<Rc<Doc>>>,
        pub layout: RefCell<Layout>,
        pub zoom: Cell<Zoom>,
        pub pages: RefCell<Cache<gdk::Texture>>,
        pub search: RefCell<Option<Search>>,
        pub selection: Cell<Option<Selection>>,
        /// Auswahlrechtecke je Seite (Punkt), bei Bedarf berechnet.
        pub selection_rects: RefCell<HashMap<usize, Vec<Rect>>>,
        pub drag_start: Cell<Option<(f64, f64)>>,
        /// Zuletzt angesprungene Seite: gilt als aktuell, bis der Benutzer
        /// selbst blättert (sonst zeigte „Gehe zu letzter Seite“ bei kleinem
        /// Zoom eine frühere Seite als aktuell).
        pub pinned: Cell<Option<usize>>,
        pub scrolling: Cell<bool>,
        /// Ziel, das vor der ersten Größenzuteilung gesetzt wurde.
        pub pending_page: Cell<Option<usize>>,
        pub changed: RefCell<Option<Callback>>,
        pub notify_queued: Cell<bool>,
        pub last_size: Cell<(i32, i32)>,
    }

    impl Default for DocView {
        fn default() -> Self {
            Self {
                hadjustment: RefCell::new(None),
                vadjustment: RefCell::new(None),
                hscroll_policy: Cell::new(gtk::ScrollablePolicy::Minimum),
                vscroll_policy: Cell::new(gtk::ScrollablePolicy::Minimum),
                doc: RefCell::new(None),
                layout: RefCell::new(Layout::default()),
                zoom: Cell::new(Zoom::FitWidth),
                pages: RefCell::new(Cache::new(cache_limit())),
                search: RefCell::new(None),
                selection: Cell::new(None),
                selection_rects: RefCell::new(HashMap::new()),
                drag_start: Cell::new(None),
                pinned: Cell::new(None),
                scrolling: Cell::new(false),
                pending_page: Cell::new(None),
                changed: RefCell::new(None),
                notify_queued: Cell::new(false),
                last_size: Cell::new((0, 0)),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DocView {
        const NAME: &'static str = "QdDocView";
        type Type = super::DocView;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("qd-view");
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for DocView {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_focusable(true);
            obj.set_overflow(gtk::Overflow::Hidden);
            obj.install_gestures();
        }
    }

    impl WidgetImpl for DocView {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (50, 300, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let obj = self.obj();
            let before = self.last_size.replace((width, height));
            if before != (width, height) {
                // Fit-Modi: Maßstab neu, oberste sichtbare Stelle bleibt.
                let anchor = obj.anchor_at_view(0.0, 0.0, before.0 as f64);
                obj.update_scale(anchor.map(|a| (a, 0.0, 0.0)));
            }
            obj.configure_adjustments();
            if let Some(page) = self.pending_page.take() {
                obj.go_to_page(page);
            }
            obj.queue_notify();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }

    impl ScrollableImpl for DocView {}

    impl DocView {
        fn set_hadjustment(&self, adj: Option<gtk::Adjustment>) {
            self.set_adjustment(&self.hadjustment, adj);
        }

        fn set_vadjustment(&self, adj: Option<gtk::Adjustment>) {
            self.set_adjustment(&self.vadjustment, adj);
        }

        fn set_adjustment(
            &self,
            slot: &RefCell<Option<gtk::Adjustment>>,
            adj: Option<gtk::Adjustment>,
        ) {
            if let Some(adj) = &adj {
                let weak = self.obj().downgrade();
                adj.connect_value_changed(move |_| {
                    if let Some(view) = weak.upgrade() {
                        let imp = view.imp();
                        if !imp.scrolling.get() {
                            imp.pinned.set(None);
                        }
                        view.queue_draw();
                        view.queue_notify();
                    }
                });
            }
            slot.replace(adj);
            self.obj().configure_adjustments();
        }
    }
}

glib::wrapper! {
    pub struct DocView(ObjectSubclass<imp::DocView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

impl Default for DocView {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// Farbe aus dem GTK-Thema; ohne Thema das Akzentblau von GTK.
fn theme_color(widget: &gtk::Widget, name: &str) -> gdk::RGBA {
    #[allow(deprecated)]
    widget
        .style_context()
        .lookup_color(name)
        .unwrap_or_else(|| gdk::RGBA::new(0.21, 0.52, 0.89, 1.0))
}

fn grect(r: Rect) -> graphene::Rect {
    graphene::Rect::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
}

impl DocView {
    /// Größe der Ansicht. Aus der letzten Größenzuteilung, nicht `width()`:
    /// Während `size_allocate` liefert GTK dort noch die alte Größe.
    fn area(&self) -> (f64, f64) {
        let (w, h) = self.imp().last_size.get();
        (f64::from(w), f64::from(h))
    }

    fn hadj(&self) -> Option<gtk::Adjustment> {
        self.imp().hadjustment.borrow().clone()
    }

    fn vadj(&self) -> Option<gtk::Adjustment> {
        self.imp().vadjustment.borrow().clone()
    }

    fn offset(&self) -> (f64, f64) {
        (
            self.hadj().map(|a| a.value()).unwrap_or(0.0),
            self.vadj().map(|a| a.value()).unwrap_or(0.0),
        )
    }

    /// Rückruf bei jeder Änderung von Bildlauf, Maßstab oder Größe; läuft im
    /// Leerlauf (nie mitten in der Größenzuteilung) und höchstens einmal je
    /// Durchgang.
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.imp().changed.replace(Some(Rc::new(f)));
    }

    fn queue_notify(&self) {
        let imp = self.imp();
        if imp.notify_queued.replace(true) {
            return;
        }
        let weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(view) = weak.upgrade() {
                view.imp().notify_queued.set(false);
                let cb = view.imp().changed.borrow().clone();
                if let Some(cb) = cb {
                    cb();
                }
            }
        });
    }

    fn configure_adjustments(&self) {
        let (w, h) = self.area();
        let l = self.imp().layout.borrow();
        let (cw, ch) = (l.content_width(), l.content_height());
        drop(l);
        let imp = self.imp();
        imp.scrolling.set(true);
        if let Some(a) = self.hadj() {
            let upper = cw.max(w);
            a.configure(
                a.value().clamp(0.0, (upper - w).max(0.0)),
                0.0,
                upper,
                STEP,
                w * 0.9,
                w,
            );
        }
        if let Some(a) = self.vadj() {
            let upper = ch.max(h);
            a.configure(
                a.value().clamp(0.0, (upper - h).max(0.0)),
                0.0,
                upper,
                STEP,
                h * 0.9,
                h,
            );
        }
        imp.scrolling.set(false);
    }

    /// Setzt einen Bildlaufwert ohne die angesprungene Seite zu vergessen.
    fn scroll_to(&self, x: Option<f64>, y: Option<f64>) {
        let imp = self.imp();
        imp.scrolling.set(true);
        if let (Some(x), Some(a)) = (x, self.hadj()) {
            a.set_value(x.clamp(0.0, (a.upper() - a.page_size()).max(0.0)));
        }
        if let (Some(y), Some(a)) = (y, self.vadj()) {
            a.set_value(y.clamp(0.0, (a.upper() - a.page_size()).max(0.0)));
        }
        imp.scrolling.set(false);
        self.queue_draw();
        self.queue_notify();
    }

    /// Neues Dokument (oder keins): Ansicht, Cache, Auswahl und Suche leeren.
    pub fn set_document(&self, doc: Option<Rc<Doc>>) {
        let imp = self.imp();
        let sizes = doc.as_ref().map(|d| d.sizes.clone()).unwrap_or_default();
        imp.layout.replace(Layout::new(sizes, 1.0));
        imp.doc.replace(doc);
        imp.pages.borrow_mut().clear();
        imp.search.replace(None);
        imp.selection.set(None);
        imp.selection_rects.borrow_mut().clear();
        imp.pinned.set(None);
        self.update_scale(None);
        self.configure_adjustments();
        self.scroll_to(Some(0.0), Some(0.0));
    }

    pub fn document(&self) -> Option<Rc<Doc>> {
        self.imp().doc.borrow().clone()
    }

    pub fn n_pages(&self) -> usize {
        self.imp().layout.borrow().len()
    }

    pub fn zoom(&self) -> Zoom {
        self.imp().zoom.get()
    }

    /// Sichtbarer Faktor (1.0 = 100 %), auch in den Fit-Modi.
    pub fn effective_factor(&self) -> f64 {
        self.imp().layout.borrow().scale() / PX_PER_PT
    }

    /// Maßstab aus Zoom und Fenstergröße; `keep` hält eine Dokumentstelle an
    /// einer Fensterstelle fest (Ankerpunkt, Fensterkoordinaten).
    fn update_scale(&self, keep: Option<(Anchor, f64, f64)>) {
        let imp = self.imp();
        let (w, h) = self.area();
        let factor = {
            let l = imp.layout.borrow();
            match imp.zoom.get() {
                Zoom::Factor(f) => f,
                _ if w < 1.0 || l.is_empty() => 1.0,
                Zoom::FitWidth => zoom::fit_width(l.max_size_pt().0, w, BORDER),
                Zoom::BestFit => zoom::best_fit(l.max_size_pt(), (w, h), BORDER),
            }
        };
        let scale = factor * PX_PER_PT;
        if (imp.layout.borrow().scale() - scale).abs() < 1e-9 {
            return;
        }
        imp.layout.borrow_mut().set_scale(scale);
        self.configure_adjustments();
        if let Some((anchor, vx, vy)) = keep {
            let (x, y) = imp.layout.borrow().point_of(anchor, w);
            self.scroll_to(Some(x - vx), Some(y - vy));
        }
        if let Some(page) = imp.pinned.get() {
            self.go_to_page(page);
        }
        self.queue_draw();
        self.queue_notify();
    }

    fn anchor_at_view(&self, vx: f64, vy: f64, area_w: f64) -> Option<Anchor> {
        let (x0, y0) = self.offset();
        self.imp()
            .layout
            .borrow()
            .anchor_at(x0 + vx, y0 + vy, area_w)
    }

    /// Zoom setzen; die Fenstermitte (oder `at`, Fensterkoordinaten) bleibt
    /// auf derselben Dokumentstelle.
    pub fn set_zoom(&self, z: Zoom, at: Option<(f64, f64)>) {
        let (w, h) = self.area();
        let (vx, vy) = at.unwrap_or((w / 2.0, h / 2.0));
        let anchor = self.anchor_at_view(vx, vy, w);
        let z = match z {
            Zoom::Factor(f) => Zoom::Factor(f.clamp(zoom::MIN, zoom::MAX)),
            other => other,
        };
        self.imp().zoom.set(z);
        self.update_scale(anchor.map(|a| (a, vx, vy)));
        self.queue_notify();
    }

    pub fn zoom_step(&self, larger: bool, at: Option<(f64, f64)>) {
        let f = self.effective_factor();
        let next = if larger {
            zoom::zoom_in(f)
        } else {
            zoom::zoom_out(f)
        };
        self.set_zoom(Zoom::Factor(next), at);
    }

    /// Maßstabsschlüssel für den Renderfaden (mit Bildschirm-Skalierung).
    pub fn render_key(&self) -> u32 {
        scale_key(self.imp().layout.borrow().scale() * f64::from(self.scale_factor().max(1)))
    }

    /// Sichtbare Seiten.
    pub fn visible_pages(&self) -> std::ops::Range<usize> {
        let (_, y0) = self.offset();
        self.imp().layout.borrow().visible(y0, y0 + self.area().1)
    }

    pub fn current_page(&self) -> usize {
        let imp = self.imp();
        if let Some(p) = imp.pinned.get().or(imp.pending_page.get()) {
            return p;
        }
        let (_, y0) = self.offset();
        imp.layout.borrow().current_page(y0, self.area().1)
    }

    pub fn go_to_page(&self, page: usize) {
        let imp = self.imp();
        let n = self.n_pages();
        if n == 0 {
            return;
        }
        let page = page.min(n - 1);
        if self.area().1 <= 1.0 {
            imp.pending_page.set(Some(page));
            return;
        }
        let y = imp.layout.borrow().scroll_target(page);
        imp.pinned.set(Some(page));
        self.scroll_to(None, Some(y));
        // ganz am Ende des Bildlaufs: Ziel ist trotzdem diese Seite
        imp.pinned.set(Some(page));
        self.queue_notify();
    }

    /// Bildlauf um ein Fenster (Leertaste) bzw. eine Pfeillänge.
    pub fn scroll_by(&self, dx: f64, dy: f64) {
        let (x0, y0) = self.offset();
        self.imp().pinned.set(None);
        self.scroll_to(Some(x0 + dx), Some(y0 + dy));
    }

    pub fn scroll_screen(&self, down: bool) {
        let h = self.area().1 * 0.9;
        self.scroll_by(0.0, if down { h } else { -h });
    }

    /// Rechteck `r` (Punkt) auf `page` sichtbar machen, mit etwas Rand.
    pub fn reveal(&self, page: usize, r: Rect) {
        let (w, h) = self.area();
        let (x0, y0) = self.offset();
        let (px, py, s) = {
            let l = self.imp().layout.borrow();
            let pr = l.page_rect(page, w);
            (pr.x, pr.y, l.scale())
        };
        let (rx, ry, rw, rh) = (px + r.x * s, py + r.y * s, r.w * s, r.h * s);
        let margin = 48.0;
        let y = if ry < y0 + margin || ry + rh > y0 + h - margin {
            Some(ry - h / 3.0)
        } else {
            None
        };
        let x = if rx < x0 || rx + rw > x0 + w {
            Some(rx - w / 3.0)
        } else {
            None
        };
        self.imp().pinned.set(None);
        if x.is_some() || y.is_some() {
            self.scroll_to(x, y);
        }
        self.queue_draw();
    }

    /// Gerendertes Bild in den Cache; neu zeichnen, wenn die Seite sichtbar ist.
    /// Sichtbare Seiten im aktuellen Maßstab bleiben dabei im Cache.
    pub fn insert_page(&self, page: usize, key: u32, texture: gdk::Texture, bytes: usize) {
        let visible = self.visible_pages();
        let current = self.render_key();
        self.imp()
            .pages
            .borrow_mut()
            .insert_protected(page, key, texture, bytes, |(p, k)| {
                *k == current && visible.contains(p)
            });
        if visible.contains(&page) {
            self.queue_draw();
        }
    }

    pub fn has_page(&self, page: usize, key: u32) -> bool {
        self.imp().pages.borrow().contains(page, key)
    }

    /// Belegter Speicher und Grenze des Seiten-Caches (Bytes), Anzahl Bilder.
    pub fn cache_usage(&self) -> (usize, usize, usize) {
        let c = self.imp().pages.borrow();
        (c.used(), c.limit(), c.len())
    }

    pub fn search_mut(&self) -> std::cell::RefMut<'_, Option<Search>> {
        self.imp().search.borrow_mut()
    }

    pub fn search(&self) -> std::cell::Ref<'_, Option<Search>> {
        self.imp().search.borrow()
    }

    pub fn selection(&self) -> Option<Selection> {
        self.imp().selection.get().filter(|s| !s.is_empty())
    }

    pub fn set_selection(&self, sel: Option<Selection>) {
        let imp = self.imp();
        imp.selection.set(sel);
        imp.selection_rects.borrow_mut().clear();
        self.queue_draw();
    }

    pub fn select_all(&self) {
        let n = self.n_pages();
        if n == 0 {
            return;
        }
        let (w, h) = self.imp().layout.borrow().page_size_pt(n - 1);
        self.set_selection(Some(Selection::all((n - 1, w, h))));
    }

    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection()?;
        let doc = self.document()?;
        Some(doc.selected_text(&sel)).filter(|t| !t.is_empty())
    }

    fn hit(&self, vx: f64, vy: f64, clamp: bool) -> Option<Point> {
        let (x0, y0) = self.offset();
        let (page, x, y) =
            self.imp()
                .layout
                .borrow()
                .hit(x0 + vx, y0 + vy, self.area().0, clamp)?;
        Some(Point { page, x, y })
    }

    fn install_gestures(&self) {
        // Ziehen markiert Text; ein einfacher Klick hebt die Auswahl auf.
        let drag = gtk::GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.downgrade();
        drag.connect_drag_begin(move |_, x, y| {
            if let Some(v) = weak.upgrade() {
                v.grab_focus();
                v.imp().drag_start.set(Some((x, y)));
                v.set_selection(None);
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(v) = weak.upgrade() else { return };
            let Some((sx, sy)) = v.imp().drag_start.get() else {
                return;
            };
            if let (Some(a), Some(b)) = (v.hit(sx, sy, true), v.hit(sx + dx, sy + dy, true)) {
                v.set_selection(Some(Selection { start: a, end: b }));
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_end(move |_, _, _| {
            let Some(v) = weak.upgrade() else { return };
            v.imp().drag_start.set(None);
            // wie unter X11: Markiertes steht für die mittlere Maustaste bereit
            if let Some(text) = v.selected_text() {
                v.primary_clipboard().set_text(&text);
            }
        });
        self.add_controller(drag);

        // Textzeiger über den Seiten
        let motion = gtk::EventControllerMotion::new();
        let weak = self.downgrade();
        motion.connect_motion(move |_, x, y| {
            if let Some(v) = weak.upgrade() {
                let over = v.hit(x, y, false).is_some();
                v.set_cursor_from_name(Some(if over { "text" } else { "default" }));
            }
        });
        self.add_controller(motion);

        // Strg+Mausrad zoomt um den Mauszeiger
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL | gtk::EventControllerScrollFlags::DISCRETE,
        );
        let pointer = Rc::new(Cell::new(None::<(f64, f64)>));
        let p = pointer.clone();
        let track = gtk::EventControllerMotion::new();
        track.connect_motion(move |_, x, y| p.set(Some((x, y))));
        self.add_controller(track);
        let weak = self.downgrade();
        scroll.connect_scroll(move |c, _, dy| {
            let Some(v) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !c
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            v.zoom_step(dy < 0.0, pointer.get());
            glib::Propagation::Stop
        });
        self.add_controller(scroll);
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let imp = self.imp();
        let (w, h) = self.area();
        let (x0, y0) = self.offset();
        let key = self.render_key();
        let widget = self.upcast_ref::<gtk::Widget>();
        let sel_color = theme_color(widget, "theme_selected_bg_color");
        let border = gdk::RGBA::new(0.25, 0.25, 0.25, 1.0);
        let shadow = gdk::RGBA::new(0.0, 0.0, 0.0, 0.25);
        let white = gdk::RGBA::WHITE;
        let layout = imp.layout.borrow();
        let scale = layout.scale();
        let search = imp.search.borrow();
        let current_hit = search.as_ref().and_then(|s| s.current_rect());
        let selection = imp.selection.get().filter(|s| !s.is_empty());
        let doc = imp.doc.borrow().clone();
        for page in layout.visible(y0, y0 + h) {
            let mut r = layout.page_rect(page, w);
            r.x -= x0;
            r.y -= y0;
            // Rahmen und Schatten wie Evince 2.30
            snapshot.append_color(
                &shadow,
                &grect(Rect::new(r.x + 2.0, r.y + 2.0, r.w + 1.0, r.h + 1.0)),
            );
            snapshot.append_color(
                &border,
                &grect(Rect::new(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0)),
            );
            match imp.pages.borrow_mut().best(page, key) {
                Some((tex, _)) => {
                    snapshot.append_scaled_texture(&tex, gsk::ScalingFilter::Linear, &grect(r));
                }
                None => snapshot.append_color(&white, &grect(r)),
            }
            let to_view = |m: &Rect| {
                Rect::new(
                    r.x + m.x * scale,
                    r.y + m.y * scale,
                    m.w * scale,
                    m.h * scale,
                )
            };
            // Suchtreffer: alle blass, der aktuelle kräftig
            if let Some(s) = search.as_ref() {
                let mut faint = sel_color;
                faint.set_alpha(0.3);
                for m in s.matches_on(page) {
                    snapshot.append_color(&faint, &grect(to_view(m)));
                }
            }
            if let Some((p, m)) = current_hit
                && p == page
            {
                let mut strong = sel_color;
                strong.set_alpha(0.6);
                snapshot.append_color(&strong, &grect(to_view(&m)));
            }
            // Textauswahl
            if let (Some(sel), Some(doc)) = (selection, doc.as_ref())
                && sel.pages().contains(&page)
            {
                let mut cache = imp.selection_rects.borrow_mut();
                let rects = cache
                    .entry(page)
                    .or_insert_with(|| doc.selection_rects(page, &sel));
                let mut c = sel_color;
                c.set_alpha(0.45);
                for m in rects.iter() {
                    snapshot.append_color(&c, &grect(to_view(m)));
                }
            }
        }
    }
}
