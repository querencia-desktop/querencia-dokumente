//! Nebenfenster: Fehlermeldung, Kennwortabfrage, Info (und das Fenster der
//! Eigenschaften, properties.rs).
//!
//! Jedes Nebenfenster wird bei jedem Öffnen neu gebaut und beim Schließen
//! zerstört, nicht verborgen: Ein verborgenes und wieder gezeigtes
//! GTK-4.22-Fenster hat in labwc 0.9.8 keine Titelleiste mehr (QD-076; labwc
//! verknüpft die Dekorationsanfrage nur mit dem ersten xdg_toplevel einer
//! Fläche). Muster Querencia Bilder 0.2.3: Der Platz je Hauptfenster hält das
//! offene Fenster nur schwach (`Slot`), ein offenes Fenster wird
//! wiederverwendet. Escape und die Knöpfe schließen im nächsten Leerlauf
//! (`close_later`).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::i18n::{gettext, gettext_f};
use crate::{APP_NAME, resource};

/// Platz für ein Nebenfenster eines Hauptfensters. Hält das offene Fenster
/// nur schwach: Nach dem Schließen gibt GTK es frei, der Platz ist dann leer
/// und `window` baut beim nächsten Öffnen ein neues.
#[derive(Clone)]
pub struct Slot(Rc<SlotInner>);

struct SlotInner {
    parent: glib::WeakRef<gtk::ApplicationWindow>,
    open: glib::WeakRef<gtk::Window>,
    modal: bool,
}

impl Slot {
    pub fn new(parent: &gtk::ApplicationWindow, modal: bool) -> Self {
        let open = glib::WeakRef::new();
        Self(Rc::new(SlotInner {
            parent: parent.downgrade(),
            open,
            modal,
        }))
    }

    /// Das offene Fenster, sonst keins.
    pub fn open(&self) -> Option<gtk::Window> {
        self.0.open.upgrade()
    }

    /// Das offene Fenster oder ein neu gebautes (leer, ohne Inhalt); keins,
    /// wenn das Hauptfenster schon zerstört ist.
    pub fn window(&self) -> Option<gtk::Window> {
        if let Some(w) = self.open() {
            return Some(w);
        }
        let parent = self.0.parent.upgrade()?;
        let w = dialog_window(&parent, self.0.modal);
        self.0.open.set(Some(&w));
        Some(w)
    }

    /// Den Platz leeren, ohne das Fenster zu schließen (es schließt sich
    /// gerade selbst); das nächste `window` baut ein neues.
    pub fn forget(&self) {
        self.0.open.set(None);
    }
}

/// Leeres Nebenfenster zu `parent`: gehört zur Anwendung wie das Hauptfenster
/// (App-ID `org.querencia.Dokumente` statt des Programmnamens, Muster
/// Querencia Bilder 0.2.3), wird mit dem Hauptfenster zerstört und beim
/// Schließen zerstört statt verborgen. Escape schließt im nächsten Leerlauf.
fn dialog_window(parent: &gtk::ApplicationWindow, modal: bool) -> gtk::Window {
    let window = gtk::Window::builder()
        .transient_for(parent)
        .modal(modal)
        .destroy_with_parent(true)
        .resizable(false)
        .build();
    if let Some(app) = parent.application() {
        window.set_application(Some(&app));
    }
    window.add_css_class("qd-dialog");
    let keys = gtk::EventControllerKey::new();
    let w = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            if let Some(w) = w.upgrade() {
                close_later(&w);
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window
}

/// Schließt (und zerstört) `window` im nächsten Leerlauf, nicht aus dem
/// laufenden Tasten- oder Klick-Handler.
pub fn close_later(window: &gtk::Window) {
    let window = window.clone();
    glib::idle_add_local_once(move || window.close());
}

/// `content` mit Rand und einem Knopf „Schließen“ rechts unten (Standard).
pub fn with_close_button(window: &gtk::Window, content: &impl IsA<gtk::Widget>) -> gtk::Box {
    let close = gtk::Button::with_mnemonic(&gettext("_Close"));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::End);
    row.set_margin_top(12);
    row.append(&close);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.set_margin_end(12);
    outer.append(content);
    outer.append(&row);
    window.set_default_widget(Some(&close));
    let w = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(w) = w.upgrade() {
            close_later(&w);
        }
    });
    outer
}

/// Inhalt setzen und den Fokus auf den Standardknopf legen, nicht auf ein
/// auswählbares Textfeld (dessen Text stünde sonst beim Öffnen markiert da).
pub fn set_content(window: &gtk::Window, content: &impl IsA<gtk::Widget>) {
    window.set_child(Some(content));
    if let Some(button) = window.default_widget() {
        GtkWindowExt::set_focus(window, Some(&button));
    }
}

/// Symbol links, fette Hauptzeile und Erklärung rechts daneben (GNOME-2-Meldung).
fn message_body(icon: &str, primary: &str, secondary: &str) -> gtk::Box {
    let img = gtk::Image::from_icon_name(icon);
    img.set_pixel_size(48);
    img.set_valign(gtk::Align::Start);
    let title = gtk::Label::new(None);
    title.set_markup(&format!(
        "<b><big>{}</big></b>",
        glib::markup_escape_text(primary)
    ));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_max_width_chars(48);
    let text = gtk::Label::new(Some(secondary));
    text.set_xalign(0.0);
    text.set_wrap(true);
    text.set_max_width_chars(56);
    text.set_selectable(true);
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.append(&title);
    col.append(&text);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.append(&img);
    row.append(&col);
    row
}

/// Fehlermeldung im offenen Fehlerfenster von `slot` oder in einem neu
/// gebauten.
pub fn show_error(slot: &Slot, primary: &str, secondary: &str) {
    let Some(window) = slot.window() else { return };
    let window = &window;
    window.set_title(Some(&gettext("Error")));
    let body = message_body("dialog-error", primary, secondary);
    set_content(window, &with_close_button(window, &body));
    window.present();
}

/// Rückruf mit dem eingegebenen Kennwort.
type UnlockCallback = Box<dyn Fn(String)>;

/// Kennwortabfrage für verschlüsselte PDFs im offenen Kennwortfenster von
/// `slot` oder in einem neu gebauten; `wrong`: der letzte Versuch schlug fehl.
/// „Dokument entsperren“ schließt das Fenster und ruft danach (im nächsten
/// Leerlauf, wenn das Fenster zerstört ist) `on_unlock` mit dem Kennwort;
/// ein neuer Versuch bekommt so ein neues Fenster.
pub fn ask_password(slot: &Slot, name: &str, wrong: bool, on_unlock: impl Fn(String) + 'static) {
    let Some(window) = slot.window() else { return };
    window.set_title(Some(&gettext("Enter password")));
    let img = gtk::Image::from_icon_name("dialog-password");
    img.set_pixel_size(48);
    img.set_valign(gtk::Align::Start);
    let title = gtk::Label::new(None);
    title.set_markup(&format!(
        "<b><big>{}</big></b>",
        glib::markup_escape_text(&gettext("Password required"))
    ));
    title.set_xalign(0.0);
    let message = gtk::Label::new(Some(&gettext_f(
        "The document “{name}” is locked and requires a password before it can be opened.",
        &[("name", name)],
    )));
    message.set_xalign(0.0);
    message.set_wrap(true);
    message.set_max_width_chars(52);
    let label = gtk::Label::with_mnemonic(&gettext("_Password:"));
    let entry = gtk::PasswordEntry::new();
    entry.set_show_peek_icon(true);
    entry.set_hexpand(true);
    entry.set_activates_default(true);
    label.set_mnemonic_widget(Some(&entry));
    let field = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    field.append(&label);
    field.append(&entry);
    let hint = gtk::Label::new(None);
    hint.set_xalign(0.0);
    hint.add_css_class("error");
    if wrong {
        hint.set_text(&gettext("Password was incorrect. Please try again."));
    }
    hint.set_visible(wrong);
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for w in [
        title.upcast_ref::<gtk::Widget>(),
        message.upcast_ref(),
        field.upcast_ref(),
        hint.upcast_ref(),
    ] {
        col.append(w);
    }
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    top.append(&img);
    top.append(&col);
    let cancel = gtk::Button::with_mnemonic(&gettext("_Cancel"));
    let unlock = gtk::Button::with_mnemonic(&gettext("_Unlock Document"));
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.set_halign(gtk::Align::End);
    buttons.set_margin_top(12);
    buttons.append(&cancel);
    buttons.append(&unlock);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.set_margin_end(12);
    outer.append(&top);
    outer.append(&buttons);
    window.set_child(Some(&outer));
    window.set_default_widget(Some(&unlock));
    let w = window.downgrade();
    cancel.connect_clicked(move |_| {
        if let Some(w) = w.upgrade() {
            close_later(&w);
        }
    });
    let on_unlock: RefCell<Option<UnlockCallback>> = RefCell::new(Some(Box::new(on_unlock)));
    let (w, e, slot) = (window.downgrade(), entry.downgrade(), slot.clone());
    unlock.connect_clicked(move |_| {
        let (Some(w), Some(e)) = (w.upgrade(), e.upgrade()) else {
            return;
        };
        let password = e.text().to_string();
        slot.forget();
        close_later(&w);
        if let Some(cb) = on_unlock.borrow_mut().take() {
            glib::idle_add_local_once(move || cb(password));
        }
    });
    window.present();
    entry.grab_focus();
}

/// „About“ („Info“): ein offenes Fenster nach vorn holen, sonst neu bauen.
pub fn show_about(slot: &Slot) {
    if let Some(w) = slot.open() {
        w.present();
        return;
    }
    let Some(window) = slot.window() else { return };
    let window = &window;
    window.set_title(Some(&gettext_f("About {name}", &[("name", APP_NAME)])));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_top(6);
    body.set_margin_start(12);
    body.set_margin_end(12);
    // Programmsymbol als SVG aus der eingebetteten Ressource (resource.rs).
    let img = gtk::Image::from_icon_name(resource::INFO_ICON);
    img.set_pixel_size(64);
    body.append(&img);
    let name = gtk::Label::new(None);
    name.set_markup(&format!(
        "<b><big>{APP_NAME} {}</big></b>",
        env!("CARGO_PKG_VERSION")
    ));
    let text = gtk::Label::new(Some(&gettext(
        "Classic document viewer for Querencia Desktop\nmodelled on Evince 2.30.\n\n\
         Documents are only displayed, never changed.\nAlpha – not released for everyday use.",
    )));
    text.set_justify(gtk::Justification::Center);
    let lic = gtk::Label::new(Some(&gettext(
        "License: GNU GPL, version 3 or later\nPDF rendering: Poppler (GPL)",
    )));
    lic.set_justify(gtk::Justification::Center);
    lic.add_css_class("dim-label");
    for w in [
        name.upcast_ref::<gtk::Widget>(),
        text.upcast_ref(),
        lic.upcast_ref(),
    ] {
        body.append(w);
    }
    set_content(window, &with_close_button(window, &body));
    window.present();
}
