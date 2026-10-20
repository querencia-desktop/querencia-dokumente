//! Querencia Dokumente – klassischer PDF-Betrachter für Querencia Desktop.
//!
//! Vorbild ist Evince 2.30 (GNOME 2): Menüleiste, Werkzeugleiste mit
//! Seitenfeld und Zoom, Seitenleiste mit Miniaturen, Suchleiste, Statusleiste.
//! GTK 4 ohne libadwaita und ohne eigene Titelleiste (labwc zeichnet die
//! Server-Titelleiste). PDF über Poppler (poppler-glib), gerendert in einem
//! Hintergrundfaden mit Cache. Oberflächentexte englisch über gettext,
//! Deutsch aus `po/de.po` (i18n.rs).
//!
//! Querencia Dokumente **schreibt nie** in Dokumente.

mod cache;
mod dialogs;
mod document;
mod i18n;
mod layout;
mod print;
mod properties;
mod recent;
mod render;
mod resource;
mod search;
mod selection;
mod sidebar;
mod view;
mod viewer;
mod zoom;

use gtk::prelude::*;
use gtk::{gio, glib};

pub const APP_ID: &str = "org.querencia.Dokumente";
pub const APP_NAME: &str = "Querencia Dokumente";

fn main() -> glib::ExitCode {
    i18n::init();
    // Name für die Liste zuletzt geöffneter Dateien (recent.rs)
    glib::set_application_name(APP_NAME);
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    app.connect_startup(|app| {
        gtk::Window::set_default_icon_name(APP_ID);
        viewer::use_classic_icon_theme();
        resource::register();
        viewer::load_css();
        // QD-081: GTK 4.22 lädt bei „prefer-dark“ das dunkle Thema-CSS nur bei
        // einem Wechsel im laufenden Programm; ein Neustart bliebe hell. Die
        // Meldung stößt das Nachladen einmal an (hell bleibt unverändert).
        if let Some(s) = gtk::Settings::default() {
            s.notify("gtk-interface-color-scheme");
        }
        viewer::install_accels(app);
        recent::init();
        app.set_menubar(Some(&viewer::menu_model()));
    });
    app.connect_activate(|app| viewer::Viewer::any(app).present());
    app.connect_open(|app, files, _| {
        for f in files {
            // erst das Fenster zeigen, dann öffnen: Fehler- und Kennwortfenster
            // brauchen ein sichtbares Elternfenster
            let v = viewer::Viewer::empty_or_new(app);
            v.present();
            v.open_file(f);
        }
    });
    app.run()
}
