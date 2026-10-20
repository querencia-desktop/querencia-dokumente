//! Das Dokumentfenster: Menüleiste, Werkzeugleiste, Seitenleiste,
//! Dokumentansicht, Suchleiste und Statusleiste.
//!
//! Menüs wie Evince 2.30: File | Edit | View | Go | Help (deutsch
//! Datei | Bearbeiten | Ansicht | Gehe zu | Hilfe aus `po/de.po`).
//! Keine eigene Titelleiste: GTK 4.22 fordert bei labwc die Server-
//! Dekoration an. Jedes Dokument bekommt ein eigenes Fenster mit eigenen
//! Hintergrundfäden (render.rs); ein leeres Fenster wird wiederverwendet.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use futures_util::StreamExt;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::dialogs;
use crate::document::{Doc, OpenError};
use crate::i18n::{gettext, gettext_f, ngettext_f, pgettext};
use crate::render::{Job, Message, Renderer, image_size};
use crate::search::Search;
use crate::sidebar::Sidebar;
use crate::view::DocView;
use crate::zoom::{self, Zoom};
use crate::{APP_NAME, print, properties, recent};

const CSS: &str = include_str!("style.css");

/// Diagnose auf stderr (Zeiten, Speicher), nie Dokumentinhalte.
fn debug() -> bool {
    std::env::var_os("QUERENCIA_DOKUMENTE_DEBUG").is_some()
}

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// GNOME Human statt Adwaita, wenn der Benutzer GTKs Standard behalten hat
/// (dieselbe Regel wie Panel, Files und Bilder).
pub fn use_classic_icon_theme() {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let current = settings
        .gtk_icon_theme_name()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let installed = glib::system_data_dirs()
        .iter()
        .any(|d| d.join("icons/gnome-human/index.theme").is_file());
    if (current.is_empty() || current == "Adwaita") && installed {
        settings.set_gtk_icon_theme_name(Some("gnome-human"));
    }
}

/// Tastenkürzel wie Evince/Atril. Kopieren und Alles auswählen sowie die
/// Blättertasten gelten nur in der Dokumentansicht (view_keys), damit sie in
/// den Eingabefeldern ihre gewohnte Wirkung behalten.
pub fn install_accels(app: &gtk::Application) {
    for (action, keys) in [
        ("win.open", &["<Control>o"][..]),
        ("win.print", &["<Control>p"]),
        ("win.properties", &["<Alt>Return"]),
        ("win.close", &["<Control>w"]),
        ("win.quit", &["<Control>q"]),
        ("win.find", &["<Control>f"]),
        ("win.find-next", &["<Control>g", "F3"]),
        ("win.find-previous", &["<Shift><Control>g", "<Shift>F3"]),
        (
            "win.zoom-in",
            &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
        ),
        ("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]),
        ("win.zoom-normal", &["<Control>0", "<Control>KP_0"]),
        ("win.fullscreen", &["F11"]),
        ("win.sidebar", &["F9"]),
        ("win.go-to-page", &["<Control>l"]),
        ("win.prev-page", &["<Control>Page_Up"]),
        ("win.next-page", &["<Control>Page_Down"]),
        ("win.first-page", &["<Control>Home"]),
        ("win.last-page", &["<Control>End"]),
    ] {
        app.set_accels_for_action(action, keys);
    }
}

/// Menüeintrag; `accel` zeigt ein Kürzel an, das nicht über die Anwendung
/// gebunden ist (es gilt nur in der Dokumentansicht).
fn item(label: &str, action: &str, accel: Option<&str>) -> gio::MenuItem {
    let i = gio::MenuItem::new(Some(label), Some(action));
    if let Some(a) = accel {
        i.set_attribute_value("accel", Some(&a.to_variant()));
    }
    i
}

fn section(items: &[gio::MenuItem]) -> gio::Menu {
    let m = gio::Menu::new();
    for i in items {
        m.append_item(i);
    }
    m
}

fn submenu(sections: &[gio::Menu]) -> gio::Menu {
    let m = gio::Menu::new();
    for s in sections {
        m.append_section(None, s);
    }
    m
}

/// Menüleiste (für alle Fenster dieselbe; die Aktionen gehören dem Fenster).
/// Zugriffstasten (_) je Menü eindeutig, in jeder Sprache für sich.
pub fn menu_model() -> gio::Menu {
    let recent = gio::Menu::new();
    recent.append_section(None, &recent::section());
    let file = gio::Menu::new();
    file.append_section(
        None,
        &section(&[item(&gettext("_Open…"), "win.open", None)]),
    );
    file.append_submenu(Some(&gettext("Open Rece_nt")), &recent);
    let rest = submenu(&[
        section(&[item(&gettext("_Print…"), "win.print", None)]),
        section(&[item(&gettext("P_roperties"), "win.properties", None)]),
        section(&[
            // TRANSLATORS: Menü „Datei“; Knöpfe in Dialogen nehmen „_Close“ ohne Kontext
            item(&pgettext("File menu", "_Close"), "win.close", None),
            item(&gettext("_Quit"), "win.quit", None),
        ]),
    ]);
    for i in 0..rest.n_items() {
        if let Some(s) = rest.item_link(i, gio::MENU_LINK_SECTION) {
            file.append_section(None, &s);
        }
    }
    let bar = gio::Menu::new();
    bar.append_submenu(Some(&gettext("_File")), &file);
    bar.append_submenu(
        Some(&gettext("_Edit")),
        &submenu(&[
            section(&[
                item(&gettext("_Copy"), "win.copy", Some("<Control>c")),
                item(
                    &gettext("Select _All"),
                    "win.select-all",
                    Some("<Control>a"),
                ),
            ]),
            section(&[
                item(&gettext("_Find…"), "win.find", None),
                item(&gettext("Find Ne_xt"), "win.find-next", None),
                item(&gettext("Find Pre_vious"), "win.find-previous", None),
            ]),
        ]),
    );
    bar.append_submenu(
        Some(&gettext("_View")),
        &submenu(&[
            section(&[
                item(&gettext("_Toolbar"), "win.toolbar", None),
                item(&gettext("_Statusbar"), "win.statusbar", None),
                item(&gettext("Side _Pane"), "win.sidebar", None),
            ]),
            section(&[
                item(&gettext("_Zoom In"), "win.zoom-in", None),
                item(&gettext("Zoom _Out"), "win.zoom-out", None),
                item(&gettext("_Normal Size"), "win.zoom-normal", None),
                item(&gettext("_Best Fit"), "win.zoom-fit-best", None),
                item(&gettext("Fit Page _Width"), "win.zoom-fit-width", None),
            ]),
            section(&[item(&gettext("_Fullscreen"), "win.fullscreen", None)]),
        ]),
    );
    bar.append_submenu(
        Some(&gettext("_Go")),
        &submenu(&[
            section(&[
                item(&gettext("_Previous Page"), "win.prev-page", None),
                item(&gettext("_Next Page"), "win.next-page", None),
            ]),
            section(&[
                item(&gettext("_First Page"), "win.first-page", None),
                item(&gettext("_Last Page"), "win.last-page", None),
            ]),
            section(&[item(&gettext("_Go to Page…"), "win.go-to-page", None)]),
        ]),
    );
    bar.append_submenu(
        Some(&gettext("_Help")),
        &submenu(&[section(&[item(&gettext("_About"), "win.about", None)])]),
    );
    bar
}

/// Menü des Zoom-Knopfs in der Werkzeugleiste.
fn zoom_menu() -> gio::Menu {
    let levels = gio::Menu::new();
    for f in zoom::MENU_LEVELS {
        let i = gio::MenuItem::new(Some(&zoom::percent(f)), None);
        i.set_action_and_target_value(Some("win.zoom-level"), Some(&f.to_variant()));
        levels.append_item(&i);
    }
    submenu(&[
        section(&[
            item(&gettext("Best Fit"), "win.zoom-fit-best", None),
            item(&gettext("Fit Page Width"), "win.zoom-fit-width", None),
        ]),
        levels,
    ])
}

fn tool_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    let b = gtk::Button::from_icon_name(icon);
    b.set_tooltip_text(Some(tooltip));
    b.set_action_name(Some(action));
    b.add_css_class("flat");
    b.set_focus_on_click(false);
    // Die Tastatur gehört der Dokumentansicht (Pfeile, Bild auf/ab).
    b.set_can_focus(false);
    b
}

/// Aktionen, die ein geöffnetes Dokument brauchen.
const DOCUMENT_ACTIONS: [&str; 18] = [
    "print",
    "properties",
    "copy",
    "select-all",
    "find",
    "find-next",
    "find-previous",
    "zoom-in",
    "zoom-out",
    "zoom-normal",
    "zoom-fit-best",
    "zoom-fit-width",
    "zoom-level",
    "prev-page",
    "next-page",
    "first-page",
    "last-page",
    "go-to-page",
];

pub struct Viewer {
    window: gtk::ApplicationWindow,
    toolbar: gtk::Box,
    statusbar: gtk::Box,
    sidebar: Rc<Sidebar>,
    stack: gtk::Stack,
    view: DocView,
    page_entry: gtk::Entry,
    page_count: gtk::Label,
    zoom_button: gtk::MenuButton,
    findbar: gtk::Box,
    find_entry: gtk::Entry,
    find_case: gtk::CheckButton,
    find_status: gtk::Label,
    status_text: gtk::Label,
    status_zoom: gtk::Label,
    status_page: gtk::Label,
    renderer: RefCell<Option<Renderer>>,
    doc_id: Cell<u64>,
    search_generation: Cell<u64>,
    search_from: Cell<usize>,
    search_started: Cell<Option<Instant>>,
    /// Beim ersten Treffer dorthin springen.
    search_jump: Cell<bool>,
    search_timer: Cell<u64>,
    failed: RefCell<HashSet<Job>>,
    toolbar_visible: Cell<bool>,
    statusbar_visible: Cell<bool>,
    sidebar_visible: Cell<bool>,
    print_settings: Rc<RefCell<Option<gtk::PrintSettings>>>,
    /// Nebenfenster, je Öffnen neu gebaut (QD-076, dialogs.rs).
    properties_window: dialogs::Slot,
    error_window: dialogs::Slot,
    about_window: dialogs::Slot,
    password_window: dialogs::Slot,
    jobs_queued: Cell<bool>,
    opened_at: Cell<Option<Instant>>,
    /// Das Fenster ist gezeichnet (siehe open_file).
    shown: Cell<bool>,
}

/// Handler einer Fensteraktion.
type ViewerAction = dyn Fn(&Rc<Viewer>);

thread_local! {
    static VIEWERS: RefCell<Vec<Rc<Viewer>>> = const { RefCell::new(Vec::new()) };
}

impl Viewer {
    /// Ein leeres Fenster (wiederverwenden) oder ein neues.
    pub fn empty_or_new(app: &gtk::Application) -> Rc<Self> {
        let empty = VIEWERS.with(|v| {
            v.borrow()
                .iter()
                .find(|w| w.view.document().is_none())
                .cloned()
        });
        empty.unwrap_or_else(|| Self::new(app))
    }

    /// Irgendein Fenster (Aktivierung ohne Datei).
    pub fn any(app: &gtk::Application) -> Rc<Self> {
        VIEWERS
            .with(|v| v.borrow().first().cloned())
            .unwrap_or_else(|| Self::new(app))
    }

    fn new(app: &gtk::Application) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(900)
            .default_height(700)
            .show_menubar(true)
            .build();
        window.add_css_class("qd-window");

        // Werkzeugleiste wie Evince 2.30
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toolbar.add_css_class("qd-toolbar");
        toolbar.append(&tool_button(
            "document-open",
            &gettext("Open a document"),
            "win.open",
        ));
        toolbar.append(&tool_button(
            "document-print",
            &gettext("Print this document"),
            "win.print",
        ));
        toolbar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        toolbar.append(&tool_button(
            "go-up",
            &gettext("Go to the previous page"),
            "win.prev-page",
        ));
        toolbar.append(&tool_button(
            "go-down",
            &gettext("Go to the next page"),
            "win.next-page",
        ));
        let page_entry = gtk::Entry::new();
        page_entry.set_width_chars(5);
        page_entry.set_max_width_chars(5);
        EntryExt::set_alignment(&page_entry, 1.0);
        page_entry.set_valign(gtk::Align::Center);
        page_entry.set_tooltip_text(Some(&gettext("Select page")));
        page_entry.set_margin_start(4);
        let page_count = gtk::Label::new(None);
        page_count.set_margin_start(4);
        page_count.set_margin_end(4);
        toolbar.append(&page_entry);
        toolbar.append(&page_count);
        toolbar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        toolbar.append(&tool_button(
            "zoom-out",
            &gettext("Shrink the document"),
            "win.zoom-out",
        ));
        toolbar.append(&tool_button(
            "zoom-in",
            &gettext("Enlarge the document"),
            "win.zoom-in",
        ));
        let zoom_button = gtk::MenuButton::new();
        zoom_button.set_menu_model(Some(&zoom_menu()));
        zoom_button.set_tooltip_text(Some(&gettext("Adjust the zoom level")));
        zoom_button.add_css_class("flat");
        zoom_button.set_can_focus(false);
        zoom_button.set_focus_on_click(false);
        toolbar.append(&zoom_button);
        toolbar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        toolbar.append(&tool_button(
            "edit-find",
            &gettext("Find a word or phrase in the document"),
            "win.find",
        ));

        // Ansicht, Seitenleiste
        let view = DocView::default();
        view.set_hexpand(true);
        view.set_vexpand(true);
        let scroller = gtk::ScrolledWindow::builder().child(&view).build();
        // klassische Rollbalken neben (nicht über) den Seiten, der senkrechte
        // immer da: „Seitenbreite“ schwankt sonst, wenn er erscheint und
        // verschwindet, und die Seite läge unter dem Balken
        scroller.set_overlay_scrolling(false);
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Always);
        scroller.add_css_class("qd-scroller");
        let sidebar = Sidebar::new();
        let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        paned.set_start_child(Some(&sidebar.root));
        paned.set_end_child(Some(&scroller));
        paned.set_resize_start_child(false);
        paned.set_shrink_start_child(false);
        paned.set_position((crate::sidebar::THUMB_W + 44.0) as i32);

        // TRANSLATORS: leeres Fenster; Menüpfad und Tastenkürzel wie in der Oberfläche
        let empty = gtk::Label::new(Some(&gettext(
            "No document loaded.\n\nFile → Open… (Ctrl+O)",
        )));
        empty.set_justify(gtk::Justification::Center);
        empty.add_css_class("dim-label");
        let stack = gtk::Stack::new();
        stack.add_named(&empty, Some("leer"));
        stack.add_named(&paned, Some("dokument"));
        stack.set_vexpand(true);

        // Suchleiste unten wie in Evince 2.30
        let findbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        findbar.add_css_class("qd-findbar");
        let close = gtk::Button::from_icon_name("window-close");
        close.add_css_class("flat");
        close.set_tooltip_text(Some(&gettext("Close the find bar")));
        close.set_action_name(Some("win.find-close"));
        let find_label = gtk::Label::with_mnemonic(&gettext("Fi_nd:"));
        let find_entry = gtk::Entry::new();
        find_entry.set_width_chars(24);
        find_label.set_mnemonic_widget(Some(&find_entry));
        let prev = gtk::Button::with_mnemonic(&gettext("Find _Previous"));
        prev.set_action_name(Some("win.find-previous"));
        prev.add_css_class("flat");
        let next = gtk::Button::with_mnemonic(&gettext("Find Ne_xt"));
        next.set_action_name(Some("win.find-next"));
        next.add_css_class("flat");
        let find_case = gtk::CheckButton::with_mnemonic(&gettext("C_ase Sensitive"));
        let find_status = gtk::Label::new(None);
        find_status.set_hexpand(true);
        find_status.set_xalign(0.0);
        for w in [
            close.upcast_ref::<gtk::Widget>(),
            find_label.upcast_ref(),
            find_entry.upcast_ref(),
            prev.upcast_ref(),
            next.upcast_ref(),
            find_case.upcast_ref(),
            find_status.upcast_ref(),
        ] {
            findbar.append(w);
        }
        findbar.set_visible(false);

        let status_text = gtk::Label::new(None);
        status_text.set_xalign(0.0);
        status_text.set_hexpand(true);
        status_text.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let status_zoom = gtk::Label::new(None);
        status_zoom.set_width_chars(7);
        let status_page = gtk::Label::new(None);
        status_page.set_width_chars(16);
        let statusbar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        statusbar.add_css_class("qd-statusbar");
        for w in [&status_text, &status_zoom, &status_page] {
            statusbar.append(w);
        }

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vbox.append(&toolbar);
        vbox.append(&stack);
        vbox.append(&findbar);
        vbox.append(&statusbar);
        window.set_child(Some(&vbox));

        let properties_window = dialogs::Slot::new(&window, false);
        let error_window = dialogs::Slot::new(&window, true);
        let about_window = dialogs::Slot::new(&window, true);
        let password_window = dialogs::Slot::new(&window, true);
        let v = Rc::new(Self {
            window,
            toolbar,
            statusbar,
            sidebar,
            stack,
            view,
            page_entry,
            page_count,
            zoom_button,
            findbar,
            find_entry,
            find_case,
            find_status,
            status_text,
            status_zoom,
            status_page,
            renderer: RefCell::new(None),
            doc_id: Cell::new(0),
            search_generation: Cell::new(0),
            search_from: Cell::new(0),
            search_started: Cell::new(None),
            search_jump: Cell::new(false),
            search_timer: Cell::new(0),
            failed: RefCell::new(HashSet::new()),
            toolbar_visible: Cell::new(true),
            statusbar_visible: Cell::new(true),
            sidebar_visible: Cell::new(true),
            print_settings: Rc::new(RefCell::new(None)),
            properties_window,
            error_window,
            about_window,
            password_window,
            jobs_queued: Cell::new(false),
            opened_at: Cell::new(None),
            shown: Cell::new(false),
        });
        v.install_actions();
        v.connect_signals();
        v.set_document_actions(false);
        v.update_status();
        VIEWERS.with(|list| list.borrow_mut().push(v.clone()));
        v
    }

    pub fn present(&self) {
        self.window.present();
        if self.view.document().is_some() {
            self.view.grab_focus();
        }
    }

    fn connect_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.view.connect_changed(move || {
            if let Some(v) = weak.upgrade() {
                v.update_status();
                v.queue_jobs();
            }
        });
        let weak = Rc::downgrade(self);
        self.sidebar.connect_page(move |page| {
            if let Some(v) = weak.upgrade() {
                v.view.go_to_page(page);
            }
        });
        let weak = Rc::downgrade(self);
        self.sidebar.connect_need(move || {
            if let Some(v) = weak.upgrade() {
                v.queue_jobs();
            }
        });
        // Seitennummer (oder Seitenbezeichnung) eingeben
        let weak = Rc::downgrade(self);
        self.page_entry.connect_activate(move |e| {
            let Some(v) = weak.upgrade() else { return };
            if let Some(page) = v.parse_page(&e.text()) {
                v.view.go_to_page(page);
            }
            v.view.grab_focus();
            v.update_status();
        });
        // Suche: Eingabe startet sie nach kurzer Pause neu, Eingabetaste springt
        let weak = Rc::downgrade(self);
        self.find_entry.connect_changed(move |_| {
            let Some(v) = weak.upgrade() else { return };
            let ticket = v.search_timer.get() + 1;
            v.search_timer.set(ticket);
            let weak = Rc::downgrade(&v);
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if let Some(v) = weak.upgrade()
                    && v.search_timer.get() == ticket
                {
                    v.start_search();
                }
            });
        });
        let weak = Rc::downgrade(self);
        self.find_entry.connect_activate(move |_| {
            if let Some(v) = weak.upgrade() {
                v.find_step(true);
            }
        });
        let weak = Rc::downgrade(self);
        self.find_case.connect_toggled(move |_| {
            if let Some(v) = weak.upgrade() {
                v.start_search();
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(v) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Escape => {
                    v.close_findbar();
                    glib::Propagation::Stop
                }
                gdk::Key::Return | gdk::Key::KP_Enter
                    if state.contains(gdk::ModifierType::SHIFT_MASK) =>
                {
                    v.find_step(false);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.find_entry.add_controller(keys);

        // Tasten der Dokumentansicht
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| match weak.upgrade() {
            Some(v) if v.view_keys(key, state) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        });
        self.view.add_controller(keys);

        // Ziehen und Ablegen von Dateien
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |_, value, _, _| {
            let (Some(v), Ok(list)) = (weak.upgrade(), value.get::<gdk::FileList>()) else {
                return false;
            };
            let files = list.files();
            let Some(app) = v.window.application() else {
                return false;
            };
            // Die erste Datei ersetzt das Dokument dieses Fensters, weitere
            // bekommen eigene Fenster.
            for (i, f) in files.iter().enumerate() {
                let target = if i == 0 {
                    v.clone()
                } else {
                    Self::empty_or_new(&app)
                };
                target.present();
                target.open_file(f);
            }
            !files.is_empty()
        });
        self.window.add_controller(drop);

        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(v) = weak.upgrade() {
                v.renderer.replace(None);
                VIEWERS.with(|list| list.borrow_mut().retain(|w| !Rc::ptr_eq(w, &v)));
                if debug() {
                    eprintln!(
                        "querencia-dokumente: Fenster geschlossen, {} offen",
                        VIEWERS.with(|l| l.borrow().len())
                    );
                }
            }
            glib::Propagation::Proceed
        });
    }

    /// Tasten der Dokumentansicht (nur wenn sie den Fokus hat).
    fn view_keys(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> bool {
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = state.contains(gdk::ModifierType::ALT_MASK);
        if alt {
            return false;
        }
        let act = |name: &str| {
            WidgetExt::activate_action(&self.window, &format!("win.{name}"), None).is_ok()
        };
        let has_doc = self.view.document().is_some();
        match key {
            gdk::Key::Escape => {
                if self.window.is_fullscreen() {
                    self.leave_fullscreen();
                } else if self.findbar.is_visible() {
                    self.close_findbar();
                } else {
                    self.view.set_selection(None);
                }
                true
            }
            _ if !has_doc => false,
            gdk::Key::c | gdk::Key::C | gdk::Key::Insert if ctrl => act("copy"),
            gdk::Key::a | gdk::Key::A if ctrl => act("select-all"),
            _ if ctrl => false,
            gdk::Key::Up | gdk::Key::KP_Up => {
                self.view.scroll_by(0.0, -48.0);
                true
            }
            gdk::Key::Down | gdk::Key::KP_Down => {
                self.view.scroll_by(0.0, 48.0);
                true
            }
            gdk::Key::Left | gdk::Key::KP_Left => {
                self.view.scroll_by(-48.0, 0.0);
                true
            }
            gdk::Key::Right | gdk::Key::KP_Right => {
                self.view.scroll_by(48.0, 0.0);
                true
            }
            gdk::Key::space => {
                self.view.scroll_screen(!shift);
                true
            }
            gdk::Key::BackSpace => {
                self.view.scroll_screen(false);
                true
            }
            gdk::Key::Page_Down | gdk::Key::KP_Page_Down => act("next-page"),
            gdk::Key::Page_Up | gdk::Key::KP_Page_Up => act("prev-page"),
            gdk::Key::Home | gdk::Key::KP_Home => act("first-page"),
            gdk::Key::End | gdk::Key::KP_End => act("last-page"),
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add => act("zoom-in"),
            gdk::Key::minus | gdk::Key::KP_Subtract => act("zoom-out"),
            gdk::Key::slash => act("find"),
            _ => false,
        }
    }

    fn parse_page(&self, text: &str) -> Option<usize> {
        let text = text.trim();
        let n = self.view.n_pages();
        if let Ok(p) = text.parse::<usize>()
            && (1..=n).contains(&p)
        {
            return Some(p - 1);
        }
        self.view.document()?.page_for_label(text)
    }

    fn show_open_error(&self, path: &Path, err: &OpenError) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let primary = gettext_f("Unable to open document “{name}”.", &[("name", &name)]);
        let detail = match err {
            OpenError::Unreadable(msg) => gettext_f(
                "The file could not be read.\n\nDetails: {detail}",
                &[("detail", msg)],
            ),
            OpenError::Damaged(msg) if msg.is_empty() => {
                gettext("The file is damaged or is not a PDF document.")
            }
            OpenError::Damaged(msg) => gettext_f(
                "The file is damaged or is not a PDF document.\n\nDetails: {detail}",
                &[("detail", msg)],
            ),
            OpenError::NeedsPassword | OpenError::WrongPassword => {
                gettext("The document is password protected.")
            }
        };
        dialogs::show_error(&self.error_window, &primary, &detail);
        if debug() {
            eprintln!("querencia-dokumente: Öffnen fehlgeschlagen: {name}: {err:?}");
        }
    }

    pub fn open_file(self: &Rc<Self>, file: &gio::File) {
        // Erst öffnen, wenn das Fenster gezeichnet ist: Kennwort- und
        // Fehlerfenster brauchen ein Elternfenster, das labwc schon kennt
        // (im map-Signal hat es noch keinen Inhalt; der Dialog erschien dann
        // verspätet und ohne Titelleiste).
        if !self.shown.get() {
            let weak = Rc::downgrade(self);
            let file = file.clone();
            let frames = Cell::new(0);
            self.window.add_tick_callback(move |_, _| {
                frames.set(frames.get() + 1);
                if frames.get() < 3 {
                    return glib::ControlFlow::Continue;
                }
                if let Some(v) = weak.upgrade() {
                    v.shown.set(true);
                    let file = file.clone();
                    glib::idle_add_local_once(move || v.open_file(&file));
                }
                glib::ControlFlow::Break
            });
            return;
        }
        match file.path() {
            Some(path) => self.load(path, None),
            None => dialogs::show_error(
                &self.error_window,
                &gettext_f(
                    "Unable to open document “{name}”.",
                    &[("name", &file.uri())],
                ),
                &gettext("Only local files are supported."),
            ),
        }
    }

    fn load(self: &Rc<Self>, path: PathBuf, password: Option<String>) {
        let started = Instant::now();
        match Doc::load(&path, password.as_deref()) {
            Ok(doc) => {
                if debug() {
                    eprintln!(
                        "querencia-dokumente: geöffnet in {} ms, {} Seiten",
                        started.elapsed().as_millis(),
                        doc.n_pages()
                    );
                }
                self.opened_at.set(Some(started));
                self.set_doc(doc);
            }
            Err(e @ (OpenError::NeedsPassword | OpenError::WrongPassword)) => {
                let wrong = e == OpenError::WrongPassword;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let weak = Rc::downgrade(self);
                dialogs::ask_password(&self.password_window, &name, wrong, move |pw| {
                    if let Some(v) = weak.upgrade() {
                        // Fokus zurück ans Dokumentfenster (labwc gibt ihn
                        // nach dem Schließen des Dialogs sonst weiter)
                        v.window.present();
                        v.load(path.clone(), Some(pw));
                    }
                });
            }
            Err(e) => self.show_open_error(&path, &e),
        }
    }

    fn set_doc(self: &Rc<Self>, doc: Doc) {
        let doc = Rc::new(doc);
        let id = self.doc_id.get() + 1;
        self.doc_id.set(id);
        self.close_findbar();
        let (tx, mut rx) = futures_channel::mpsc::unbounded();
        self.renderer.replace(Some(Renderer::start(
            doc.path.clone(),
            doc.password.clone(),
            tx,
        )));
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Some(msg) = rx.next().await {
                let Some(v) = weak.upgrade() else { break };
                if v.doc_id.get() != id {
                    break;
                }
                v.handle(msg);
            }
        });
        self.failed.borrow_mut().clear();
        let labels: Vec<String> = (0..doc.n_pages())
            .map(|i| {
                doc.poppler
                    .page(i as i32)
                    .and_then(|p| p.label())
                    .map(|l| l.to_string())
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| (i + 1).to_string())
            })
            .collect();
        self.sidebar.set_document(&labels, &doc.sizes);
        self.view.set_document(Some(doc.clone()));
        self.stack.set_visible_child_name("dokument");
        let name = doc.title().unwrap_or_else(|| doc.file_name());
        self.window.set_title(Some(&format!("{name} – {APP_NAME}")));
        self.status_text.set_text(&doc.file_name());
        recent::add(&gio::File::for_path(&doc.path));
        self.set_document_actions(true);
        self.update_status();
        self.queue_jobs();
        self.view.grab_focus();
    }

    fn handle(self: &Rc<Self>, msg: Message) {
        match msg {
            Message::Rendered(img) => {
                let bytes = img.data.len();
                let texture: gdk::Texture = gdk::MemoryTexture::new(
                    img.width,
                    img.height,
                    gdk::MemoryFormat::B8g8r8a8Premultiplied,
                    &img.data,
                    img.stride,
                )
                .upcast();
                let job = img.job;
                if job.thumb {
                    self.sidebar.insert(job.page, job.key, texture, bytes);
                } else {
                    self.view.insert_page(job.page, job.key, texture, bytes);
                    if let Some(t) = self.opened_at.take()
                        && debug()
                    {
                        eprintln!(
                            "querencia-dokumente: erste Seite nach {} ms",
                            t.elapsed().as_millis()
                        );
                    }
                    if debug() {
                        let (used, limit, n) = self.view.cache_usage();
                        eprintln!(
                            "querencia-dokumente: Seite {} ({}×{}), Cache {} Bilder, {} von {} MB",
                            job.page + 1,
                            img.width,
                            img.height,
                            n,
                            used / 1_048_576,
                            limit / 1_048_576
                        );
                    }
                }
            }
            Message::Failed(job) => {
                if debug() {
                    eprintln!(
                        "querencia-dokumente: Seite {} nicht gerendert",
                        job.page + 1
                    );
                }
                self.failed.borrow_mut().insert(job);
            }
            Message::Found {
                generation,
                page,
                rects,
            } => {
                if generation != self.search_generation.get() {
                    return;
                }
                let target = {
                    let mut s = self.view.search_mut();
                    let Some(s) = s.as_mut() else { return };
                    let hits = !rects.is_empty();
                    s.add_page(page, rects);
                    if self.search_jump.get() && hits {
                        self.search_jump.set(false);
                        s.next(self.search_from.get());
                        s.current_rect()
                    } else {
                        None
                    }
                };
                if let Some((p, r)) = target {
                    self.view.reveal(p, r);
                }
                if self.view.visible_pages().contains(&page) {
                    self.view.queue_draw();
                }
                self.update_find_status();
            }
            Message::SearchDone { generation } => {
                if generation != self.search_generation.get() {
                    return;
                }
                if let Some(s) = self.view.search_mut().as_mut() {
                    s.finish();
                }
                if debug()
                    && let Some(t) = self.search_started.take()
                {
                    let n = self.view.search().as_ref().map(Search::count).unwrap_or(0);
                    eprintln!(
                        "querencia-dokumente: Suche über {} Seiten in {} ms, {n} Treffer",
                        self.view.n_pages(),
                        t.elapsed().as_millis()
                    );
                }
                self.update_find_status();
            }
        }
    }

    /// Renderaufträge neu bestimmen (im Leerlauf, höchstens einmal je Durchgang).
    fn queue_jobs(self: &Rc<Self>) {
        if self.jobs_queued.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(v) = weak.upgrade() {
                v.jobs_queued.set(false);
                v.update_jobs();
            }
        });
    }

    /// Sichtbare Seiten zuerst, dann (wenn der Cache sie fasst) die
    /// Nachbarseiten, dann die sichtbaren Miniaturen.
    fn update_jobs(&self) {
        let renderer = self.renderer.borrow();
        let (Some(r), Some(doc)) = (renderer.as_ref(), self.view.document()) else {
            return;
        };
        let failed = self.failed.borrow();
        let key = self.view.render_key();
        let scale = f64::from(key) / 1000.0;
        let n = doc.n_pages();
        let visible = self.view.visible_pages();
        let mut jobs = Vec::new();
        let page_job = |page: usize| Job {
            page,
            key,
            thumb: false,
        };
        let bytes_of = |page: usize| {
            let (w, h) = image_size(doc.sizes[page], scale);
            4 * w as usize * h as usize
        };
        let mut budget: usize = 0;
        for page in visible.clone() {
            budget += bytes_of(page);
            let j = page_job(page);
            if !self.view.has_page(page, key) && !failed.contains(&j) {
                jobs.push(j);
            }
        }
        let (_, limit, _) = self.view.cache_usage();
        let mut neighbours = vec![visible.end, visible.end + 1];
        if let Some(p) = visible.start.checked_sub(1) {
            neighbours.push(p);
        }
        for page in neighbours.into_iter().filter(|p| *p < n) {
            budget += bytes_of(page);
            let j = page_job(page);
            if budget <= limit / 2 && !self.view.has_page(page, key) && !failed.contains(&j) {
                jobs.push(j);
            }
        }
        let tkey = self.sidebar.key();
        for page in self.sidebar.missing() {
            let j = Job {
                page,
                key: tkey,
                thumb: true,
            };
            if !failed.contains(&j) {
                jobs.push(j);
            }
        }
        r.set_jobs(jobs);
    }

    fn update_status(&self) {
        let n = self.view.n_pages();
        if n == 0 {
            self.page_entry.set_text("");
            self.page_entry.set_sensitive(false);
            self.page_count.set_text("");
            self.status_page.set_text("");
            self.status_zoom.set_text("");
            self.zoom_button.set_label(&zoom::percent(1.0));
            self.zoom_button.set_sensitive(false);
            return;
        }
        self.page_entry.set_sensitive(true);
        self.zoom_button.set_sensitive(true);
        let page = self.view.current_page();
        if !self.page_entry.has_focus() {
            self.page_entry.set_text(&(page + 1).to_string());
        }
        // TRANSLATORS: hinter dem Seitenfeld der Werkzeugleiste („[3] of 12“)
        self.page_count
            .set_text(&gettext_f("of {count}", &[("count", &n.to_string())]));
        self.status_page.set_text(&gettext_f(
            // TRANSLATORS: Statusleiste, aktuelle Seite
            "Page {index} of {count}",
            &[
                ("index", &(page + 1).to_string()),
                ("count", &n.to_string()),
            ],
        ));
        let pct = zoom::percent(self.view.effective_factor());
        self.status_zoom.set_text(&pct);
        self.zoom_button.set_label(&match self.view.zoom() {
            Zoom::BestFit => gettext("Best Fit"),
            Zoom::FitWidth => gettext("Fit Page Width"),
            Zoom::Factor(_) => pct,
        });
        self.sidebar.show_page(page);
        self.enable("prev-page", page > 0);
        self.enable("first-page", page > 0);
        self.enable("next-page", page + 1 < n);
        self.enable("last-page", page + 1 < n);
        let f = self.view.effective_factor();
        self.enable("zoom-in", f < zoom::MAX - 1e-6);
        self.enable("zoom-out", f > zoom::MIN + 1e-6);
    }

    fn enable(&self, name: &str, on: bool) {
        if let Some(a) = self
            .window
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        {
            a.set_enabled(on);
        }
    }

    fn set_document_actions(&self, on: bool) {
        for name in DOCUMENT_ACTIONS {
            self.enable(name, on);
        }
    }

    // --- Suche ---------------------------------------------------------------

    fn open_findbar(&self) {
        self.findbar.set_visible(true);
        self.find_entry.grab_focus();
        self.find_entry.select_region(0, -1);
    }

    fn close_findbar(&self) {
        if let Some(r) = self.renderer.borrow().as_ref() {
            r.cancel_search();
        }
        self.search_generation.set(self.search_generation.get() + 1);
        self.view.search_mut().take();
        self.view.queue_draw();
        self.find_status.set_text("");
        if self.findbar.is_visible() {
            self.findbar.set_visible(false);
            self.view.grab_focus();
        }
    }

    fn start_search(self: &Rc<Self>) {
        let text = self.find_entry.text().to_string();
        let renderer = self.renderer.borrow();
        let Some(r) = renderer.as_ref() else { return };
        if text.is_empty() {
            r.cancel_search();
            self.search_generation.set(self.search_generation.get() + 1);
            self.view.search_mut().take();
            self.view.queue_draw();
            self.find_status.set_text("");
            return;
        }
        let case = self.find_case.is_active();
        let from = self.view.current_page();
        self.search_from.set(from);
        self.search_jump.set(true);
        self.search_started.set(Some(Instant::now()));
        let generation = r.search(&text, case, from);
        self.search_generation.set(generation);
        self.view
            .search_mut()
            .replace(Search::new(&text, case, self.view.n_pages()));
        self.view.queue_draw();
        self.update_find_status();
    }

    /// Weiter (`forward`) oder zurück zum nächsten Treffer; startet die Suche,
    /// wenn sich Text oder Groß-/Kleinschreibung geändert haben.
    fn find_step(self: &Rc<Self>, forward: bool) {
        if !self.findbar.is_visible() {
            self.open_findbar();
        }
        let text = self.find_entry.text().to_string();
        if text.is_empty() {
            return;
        }
        let stale = self
            .view
            .search()
            .as_ref()
            .map(|s| s.query != text || s.case_sensitive != self.find_case.is_active())
            .unwrap_or(true);
        if stale {
            self.start_search();
            return;
        }
        let from = self.view.current_page();
        let target = {
            let mut s = self.view.search_mut();
            s.as_mut().and_then(|s| {
                if forward { s.next(from) } else { s.prev(from) }?;
                s.current_rect()
            })
        };
        if let Some((p, r)) = target {
            self.search_jump.set(false);
            self.view.reveal(p, r);
        }
        self.update_find_status();
    }

    fn update_find_status(&self) {
        let s = self.view.search();
        let Some(s) = s.as_ref() else {
            self.find_status.set_text("");
            return;
        };
        let text = match (s.count(), s.current_number(), s.is_done()) {
            (0, _, false) => gettext("Searching…"),
            (0, _, true) => gettext("Not found"),
            (count, Some(i), _) => gettext_f(
                // TRANSLATORS: Suchleiste, aktueller Treffer
                "Match {index} of {count}",
                &[("index", &i.to_string()), ("count", &count.to_string())],
            ),
            (count, None, _) => ngettext_f("{n} match", "{n} matches", count as u32, &[]),
        };
        self.find_status.set_text(&text);
    }

    // --- Vollbild ------------------------------------------------------------

    fn enter_fullscreen(&self) {
        self.window.set_show_menubar(false);
        self.toolbar.set_visible(false);
        self.statusbar.set_visible(false);
        self.sidebar.root.set_visible(false);
        self.window.add_css_class("qd-fullscreen");
        self.window.fullscreen();
    }

    fn leave_fullscreen(&self) {
        self.window.unfullscreen();
        self.window.remove_css_class("qd-fullscreen");
        self.window.set_show_menubar(true);
        self.toolbar.set_visible(self.toolbar_visible.get());
        self.statusbar.set_visible(self.statusbar_visible.get());
        self.sidebar.root.set_visible(self.sidebar_visible.get());
    }

    // --- Aktionen ------------------------------------------------------------

    fn install_actions(self: &Rc<Self>) {
        let add = |name: &str, f: Box<ViewerAction>| {
            let a = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            a.connect_activate(move |_, _| {
                if let Some(v) = weak.upgrade() {
                    f(&v);
                }
            });
            self.window.add_action(&a);
        };
        add("open", Box::new(|v| v.open_dialog()));
        add(
            "print",
            Box::new(|v| {
                if let Some(doc) = v.view.document() {
                    print::run(
                        &v.window,
                        doc,
                        v.print_settings.clone(),
                        v.error_window.clone(),
                    );
                }
            }),
        );
        add(
            "properties",
            Box::new(|v| {
                if let Some(doc) = v.view.document() {
                    properties::show(&v.properties_window, &doc);
                }
            }),
        );
        add("close", Box::new(|v| v.window.close()));
        add(
            "quit",
            Box::new(|v| {
                let windows: Vec<Rc<Viewer>> = VIEWERS.with(|l| l.borrow().clone());
                for w in windows {
                    w.window.close();
                }
                if let Some(app) = v.window.application() {
                    app.quit();
                }
            }),
        );
        add(
            "copy",
            Box::new(|v| {
                if let Some(text) = v.view.selected_text() {
                    v.window.clipboard().set_text(&text);
                }
            }),
        );
        add("select-all", Box::new(|v| v.view.select_all()));
        add("find", Box::new(|v| v.open_findbar()));
        add("find-close", Box::new(|v| v.close_findbar()));
        add("find-next", Box::new(|v| v.find_step(true)));
        add("find-previous", Box::new(|v| v.find_step(false)));
        add("zoom-in", Box::new(|v| v.view.zoom_step(true, None)));
        add("zoom-out", Box::new(|v| v.view.zoom_step(false, None)));
        add(
            "zoom-normal",
            Box::new(|v| v.view.set_zoom(Zoom::Factor(1.0), None)),
        );
        add(
            "zoom-fit-best",
            Box::new(|v| v.view.set_zoom(Zoom::BestFit, None)),
        );
        add(
            "zoom-fit-width",
            Box::new(|v| v.view.set_zoom(Zoom::FitWidth, None)),
        );
        let level = gio::SimpleAction::new("zoom-level", Some(glib::VariantTy::DOUBLE));
        let weak = Rc::downgrade(self);
        level.connect_activate(move |_, p| {
            if let (Some(v), Some(f)) = (weak.upgrade(), p.and_then(|p| p.get::<f64>())) {
                v.view.set_zoom(Zoom::Factor(f), None);
            }
        });
        self.window.add_action(&level);
        let recent = gio::SimpleAction::new("open-recent", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(self);
        recent.connect_activate(move |_, p| {
            let (Some(v), Some(uri)) = (weak.upgrade(), p.and_then(|p| p.get::<String>())) else {
                return;
            };
            let file = gio::File::for_uri(&uri);
            let target = match v.window.application() {
                Some(app) if v.view.document().is_some() => Self::empty_or_new(&app),
                _ => v.clone(),
            };
            target.present();
            target.open_file(&file);
        });
        self.window.add_action(&recent);
        add(
            "fullscreen",
            Box::new(|v| {
                if v.window.is_fullscreen() {
                    v.leave_fullscreen();
                } else {
                    v.enter_fullscreen();
                }
            }),
        );
        add(
            "prev-page",
            Box::new(|v| {
                let p = v.view.current_page();
                v.view.go_to_page(p.saturating_sub(1));
            }),
        );
        add(
            "next-page",
            Box::new(|v| {
                let p = v.view.current_page();
                v.view.go_to_page(p + 1);
            }),
        );
        add("first-page", Box::new(|v| v.view.go_to_page(0)));
        add(
            "last-page",
            Box::new(|v| v.view.go_to_page(v.view.n_pages().saturating_sub(1))),
        );
        add(
            "go-to-page",
            Box::new(|v| {
                if !v.toolbar.is_visible() {
                    v.toolbar.set_visible(true);
                    v.toolbar_visible.set(true);
                    v.sync_toggle("toolbar", true);
                }
                v.page_entry.grab_focus();
                v.page_entry.select_region(0, -1);
            }),
        );
        add(
            "about",
            Box::new(|v| {
                dialogs::show_about(&v.about_window);
            }),
        );
        for name in ["toolbar", "statusbar", "sidebar"] {
            let a = gio::SimpleAction::new_stateful(name, None, &true.to_variant());
            let weak = Rc::downgrade(self);
            a.connect_activate(move |a, _| {
                let Some(v) = weak.upgrade() else { return };
                let on = !a.state().and_then(|s| s.get::<bool>()).unwrap_or(true);
                a.set_state(&on.to_variant());
                match a.name().as_str() {
                    "toolbar" => {
                        v.toolbar_visible.set(on);
                        v.toolbar.set_visible(on);
                    }
                    "statusbar" => {
                        v.statusbar_visible.set(on);
                        v.statusbar.set_visible(on);
                    }
                    _ => {
                        v.sidebar_visible.set(on);
                        v.sidebar.root.set_visible(on);
                        v.queue_jobs();
                    }
                }
            });
            self.window.add_action(&a);
        }
    }

    fn sync_toggle(&self, name: &str, on: bool) {
        if let Some(a) = self
            .window
            .lookup_action(name)
            .and_downcast::<gio::SimpleAction>()
        {
            a.set_state(&on.to_variant());
        }
    }

    fn open_dialog(self: &Rc<Self>) {
        let pdf = gtk::FileFilter::new();
        pdf.set_name(Some(&gettext("PDF Documents")));
        pdf.add_mime_type("application/pdf");
        pdf.add_suffix("pdf");
        let all = gtk::FileFilter::new();
        all.set_name(Some(&gettext("All Files")));
        all.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&pdf);
        filters.append(&all);
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Open Document"))
            .modal(true)
            .filters(&filters)
            .default_filter(&pdf)
            .build();
        if let Some(dir) = self
            .view
            .document()
            .and_then(|d| gio::File::for_path(&d.path).parent())
        {
            dialog.set_initial_folder(Some(&dir));
        }
        let weak = Rc::downgrade(self);
        dialog.open_multiple(Some(&self.window), gio::Cancellable::NONE, move |r| {
            let (Some(v), Ok(model)) = (weak.upgrade(), r) else {
                return;
            };
            let files: Vec<gio::File> = (0..model.n_items())
                .filter_map(|i| model.item(i).and_downcast())
                .collect();
            let Some(app) = v.window.application() else {
                return;
            };
            for (i, f) in files.iter().enumerate() {
                let target = if i == 0 && v.view.document().is_none() {
                    v.clone()
                } else {
                    Self::empty_or_new(&app)
                };
                target.present();
                target.open_file(f);
            }
        });
    }
}
