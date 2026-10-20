//! Nebenfenster: Fehlermeldung, Kennwortabfrage, Info.
//!
//! Jedes Fenster wird je Hauptfenster einmal angelegt und beim Schließen nur
//! verborgen (wie in Querencia Bilder): GTK 4.22.4 stürzt beim Zerstören
//! eines Fensters ab, das aus einem Tastenkürzel heraus geöffnet wurde
//! (SIGSEGV in libwayland-client, QI-01). Escape schließt jedes dieser
//! Fenster.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::i18n::{gettext, gettext_f};
use crate::{APP_NAME, resource};

/// Leeres Nebenfenster zu `parent`, verborgen statt zerstört.
pub fn window(parent: &gtk::ApplicationWindow, modal: bool) -> gtk::Window {
    let window = gtk::Window::builder()
        .transient_for(parent)
        .modal(modal)
        .hide_on_close(true)
        .resizable(false)
        .build();
    window.add_css_class("qd-dialog");
    let keys = gtk::EventControllerKey::new();
    let w = window.downgrade();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            if let Some(w) = w.upgrade() {
                w.close();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window
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
            w.close();
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

/// Fehlermeldung im (wiederverwendeten) Fenster `window`.
pub fn show_error(window: &gtk::Window, primary: &str, secondary: &str) {
    window.set_title(Some(&gettext("Error")));
    let body = message_body("dialog-error", primary, secondary);
    set_content(window, &with_close_button(window, &body));
    window.present();
}

/// Rückruf mit dem eingegebenen Kennwort.
type UnlockCallback = Box<dyn Fn(String)>;

/// Kennwortabfrage für verschlüsselte PDFs.
pub struct PasswordDialog {
    window: gtk::Window,
    message: gtk::Label,
    hint: gtk::Label,
    entry: gtk::PasswordEntry,
    on_unlock: RefCell<Option<UnlockCallback>>,
}

impl PasswordDialog {
    pub fn new(parent: &gtk::ApplicationWindow) -> Rc<Self> {
        let window = window(parent, true);
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
        let message = gtk::Label::new(None);
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
        let d = Rc::new(Self {
            window,
            message,
            hint,
            entry,
            on_unlock: RefCell::new(None),
        });
        let w = d.window.downgrade();
        cancel.connect_clicked(move |_| {
            if let Some(w) = w.upgrade() {
                w.close();
            }
        });
        let weak = Rc::downgrade(&d);
        unlock.connect_clicked(move |_| {
            let Some(d) = weak.upgrade() else { return };
            let password = d.entry.text().to_string();
            d.window.close();
            let cb = d.on_unlock.borrow_mut().take();
            if let Some(cb) = cb {
                cb(password);
            }
        });
        d
    }

    /// Fragt nach dem Kennwort für `name`; `wrong`: der letzte Versuch schlug fehl.
    pub fn ask(&self, name: &str, wrong: bool, on_unlock: impl Fn(String) + 'static) {
        self.message.set_text(&gettext_f(
            "The document “{name}” is locked and requires a password before it can be opened.",
            &[("name", name)],
        ));
        self.hint.set_text(&if wrong {
            gettext("Password was incorrect. Please try again.")
        } else {
            String::new()
        });
        self.hint.set_visible(wrong);
        self.entry.set_text("");
        self.on_unlock.replace(Some(Box::new(on_unlock)));
        self.window.present();
        self.entry.grab_focus();
    }
}

/// „About“ („Info“) im (wiederverwendeten) Fenster `window`.
pub fn show_about(window: &gtk::Window) {
    window.set_title(Some(&gettext_f("About {name}", &[("name", APP_NAME)])));
    if window.child().is_none() {
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
    }
    window.present();
}
