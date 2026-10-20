//! Seitenleiste mit Miniaturen (F9). Eine `gtk::ListView`: Es gibt nur
//! Widgets für die sichtbaren Zeilen, und nur für diese werden Miniaturen
//! gerendert (Hintergrundfaden, eigener Cache mit eigener Obergrenze).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

use crate::cache::{Cache, scale_key};

/// Breite der breitesten Seite als Miniatur (Pixel).
pub const THUMB_W: f64 = 110.0;
/// Obergrenze des Miniaturen-Caches (≈ 500 A4-Miniaturen).
const THUMB_CACHE: usize = 32 * 1024 * 1024;

type PageCallback = Box<dyn Fn(usize)>;

pub struct Sidebar {
    pub root: gtk::ScrolledWindow,
    list: gtk::ListView,
    model: gtk::StringList,
    selection: gtk::SingleSelection,
    cache: RefCell<Cache<gdk::Texture>>,
    /// Position → Bild der gerade gebundenen Zeile.
    bound: RefCell<HashMap<u32, gtk::Picture>>,
    sizes: RefCell<Vec<(f64, f64)>>,
    scale: Cell<f64>,
    syncing: Cell<bool>,
    on_page: RefCell<Option<PageCallback>>,
    on_need: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Sidebar {
    pub fn new() -> Rc<Self> {
        let model = gtk::StringList::new(&[]);
        let selection = gtk::SingleSelection::new(Some(model.clone()));
        let factory = gtk::SignalListItemFactory::new();
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory.clone()));
        list.add_css_class("qd-thumbnails");
        let root = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        root.set_size_request((THUMB_W + 40.0) as i32, -1);
        let s = Rc::new(Self {
            root,
            list,
            model,
            selection,
            cache: RefCell::new(Cache::new(THUMB_CACHE)),
            bound: RefCell::new(HashMap::new()),
            sizes: RefCell::new(Vec::new()),
            scale: Cell::new(1.0),
            syncing: Cell::new(false),
            on_page: RefCell::new(None),
            on_need: RefCell::new(None),
        });
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let picture = gtk::Picture::new();
            picture.set_can_shrink(false);
            picture.set_halign(gtk::Align::Center);
            picture.add_css_class("qd-thumb");
            let label = gtk::Label::new(None);
            let b = gtk::Box::new(gtk::Orientation::Vertical, 3);
            b.set_margin_top(6);
            b.set_margin_bottom(6);
            b.append(&picture);
            b.append(&label);
            item.set_child(Some(&b));
        });
        let weak = Rc::downgrade(&s);
        factory.connect_bind(move |_, item| {
            let (Some(s), Some(item)) = (weak.upgrade(), item.downcast_ref::<gtk::ListItem>())
            else {
                return;
            };
            let Some((picture, label)) = parts(item) else {
                return;
            };
            let pos = item.position();
            if let Some(text) = item.item().and_downcast::<gtk::StringObject>() {
                label.set_text(&text.string());
            }
            let (w, h) = s.thumb_size(pos as usize);
            picture.set_size_request(w, h);
            let key = s.key();
            picture.set_paintable(s.cache.borrow_mut().get(pos as usize, key).as_ref());
            s.bound.borrow_mut().insert(pos, picture);
            if !s.cache.borrow().contains(pos as usize, key) {
                let cb = s.on_need.borrow().clone();
                if let Some(cb) = cb {
                    cb();
                }
            }
        });
        let weak = Rc::downgrade(&s);
        factory.connect_unbind(move |_, item| {
            let (Some(s), Some(item)) = (weak.upgrade(), item.downcast_ref::<gtk::ListItem>())
            else {
                return;
            };
            let pos = item.position();
            let mut bound = s.bound.borrow_mut();
            let same = match (bound.get(&pos), parts(item)) {
                (Some(p), Some((q, _))) => *p == q,
                _ => false,
            };
            if same {
                bound.remove(&pos);
            }
        });
        let weak = Rc::downgrade(&s);
        s.selection.connect_selected_notify(move |sel| {
            let Some(s) = weak.upgrade() else { return };
            if s.syncing.get() || sel.selected() == gtk::INVALID_LIST_POSITION {
                return;
            }
            if let Some(cb) = s.on_page.borrow().as_ref() {
                cb(sel.selected() as usize);
            }
        });
        let weak = Rc::downgrade(&s);
        s.list.connect_activate(move |_, pos| {
            if let Some(s) = weak.upgrade()
                && let Some(cb) = s.on_page.borrow().as_ref()
            {
                cb(pos as usize);
            }
        });
        s
    }

    /// Rückruf, wenn der Benutzer eine Miniatur wählt.
    pub fn connect_page(&self, f: impl Fn(usize) + 'static) {
        self.on_page.replace(Some(Box::new(f)));
    }

    /// Rückruf, wenn sichtbare Miniaturen fehlen.
    pub fn connect_need(&self, f: impl Fn() + 'static) {
        self.on_need.replace(Some(Rc::new(f)));
    }

    pub fn set_document(&self, labels: &[String], sizes: &[(f64, f64)]) {
        self.cache.borrow_mut().clear();
        self.bound.borrow_mut().clear();
        let max_w = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
        self.scale
            .set(if max_w > 0.0 { THUMB_W / max_w } else { 1.0 });
        self.sizes.replace(sizes.to_vec());
        self.syncing.set(true);
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        self.model.splice(0, self.model.n_items(), &labels);
        self.syncing.set(false);
    }

    fn thumb_size(&self, page: usize) -> (i32, i32) {
        let (w, h) = self.sizes.borrow().get(page).copied().unwrap_or((0.0, 0.0));
        let s = self.scale.get();
        ((w * s).round() as i32, (h * s).round() as i32)
    }

    /// Maßstabsschlüssel der Miniaturen (Pixel je Punkt, mit Skalierung).
    pub fn key(&self) -> u32 {
        scale_key(self.scale.get() * f64::from(self.root.scale_factor().max(1)))
    }

    /// Gebundene (sichtbare) Seiten ohne Miniatur, von oben nach unten.
    pub fn missing(&self) -> Vec<usize> {
        if !self.root.is_visible() {
            return Vec::new();
        }
        let key = self.key();
        let cache = self.cache.borrow();
        let mut pages: Vec<usize> = self
            .bound
            .borrow()
            .keys()
            .map(|p| *p as usize)
            .filter(|p| !cache.contains(*p, key))
            .collect();
        pages.sort_unstable();
        pages
    }

    pub fn insert(&self, page: usize, key: u32, texture: gdk::Texture, bytes: usize) {
        if let Some(p) = self.bound.borrow().get(&(page as u32))
            && key == self.key()
        {
            p.set_paintable(Some(&texture));
        }
        self.cache.borrow_mut().insert(page, key, texture, bytes);
    }

    /// Zeigt `page` als gewählt (ohne zu blättern) und rollt die Liste hin.
    pub fn show_page(&self, page: usize) {
        if page as u32 >= self.model.n_items() || self.selection.selected() == page as u32 {
            return;
        }
        self.syncing.set(true);
        self.selection.set_selected(page as u32);
        self.syncing.set(false);
        if self.root.is_visible() {
            self.list
                .scroll_to(page as u32, gtk::ListScrollFlags::NONE, None);
        }
    }
}

fn parts(item: &gtk::ListItem) -> Option<(gtk::Picture, gtk::Label)> {
    let b = item.child()?;
    let picture = b.first_child()?.downcast::<gtk::Picture>().ok()?;
    let label = b.last_child()?.downcast::<gtk::Label>().ok()?;
    Some((picture, label))
}
