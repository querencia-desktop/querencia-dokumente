//! Zuletzt geöffnete Dokumente (Menü „Datei → Zuletzt geöffnet“) aus der
//! gemeinsamen Liste von GTK (`recently-used.xbel`, wie Evince und Files):
//! nur Einträge, die Querencia Dokumente selbst eingetragen hat und die es
//! noch gibt, neueste zuerst.

use gtk::gio;
use gtk::prelude::*;

use crate::APP_NAME;
use crate::i18n::{gettext, gettext_f};

/// Höchstens so viele Einträge im Menü.
const MAX_ITEMS: usize = 8;

thread_local! {
    static SECTION: gio::Menu = gio::Menu::new();
}

/// Abschnitt mit den Einträgen (wird bei jeder Änderung neu gefüllt).
pub fn section() -> gio::Menu {
    SECTION.with(Clone::clone)
}

pub fn init() {
    let manager = gtk::RecentManager::default();
    manager.connect_changed(rebuild);
    rebuild(&manager);
}

/// Dokument eintragen (nach erfolgreichem Öffnen).
pub fn add(file: &gio::File) {
    let uri = file.uri();
    let data = gtk::RecentData::new(
        None,
        None,
        "application/pdf",
        APP_NAME,
        "querencia-dokumente %u",
        &[],
        false,
    );
    gtk::RecentManager::default().add_full(&uri, &data);
}

/// Unterstriche im Dateinamen verdoppeln, sonst würden sie zur Zugriffstaste.
pub fn escape_mnemonic(name: &str) -> String {
    name.replace('_', "__")
}

fn rebuild(manager: &gtk::RecentManager) {
    let mut items: Vec<gtk::RecentInfo> = manager
        .items()
        .into_iter()
        .filter(|i| i.has_application(APP_NAME) && i.is_local() && i.exists())
        .collect();
    items.sort_by_key(|i| std::cmp::Reverse(i.modified().to_unix()));
    let section = section();
    section.remove_all();
    for (n, info) in items.iter().take(MAX_ITEMS).enumerate() {
        let label = gettext_f(
            // TRANSLATORS: Eintrag im Menü „Zuletzt geöffnet“: Nummer und Dateiname
            "_{number}. {name}",
            &[
                ("number", &(n + 1).to_string()),
                ("name", &escape_mnemonic(&info.display_name())),
            ],
        );
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(
            Some("win.open-recent"),
            Some(&info.uri().to_string().to_variant()),
        );
        section.append_item(&item);
    }
    if items.is_empty() {
        // Aktion ohne Ziel: Der Eintrag erscheint ausgegraut.
        section.append(Some(&gettext("No recent documents")), Some("win.no-recent"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn underscores_do_not_become_mnemonics() {
        assert_eq!(
            escape_mnemonic("mein_bericht_2.pdf"),
            "mein__bericht__2.pdf"
        );
        assert_eq!(escape_mnemonic("plain.pdf"), "plain.pdf");
    }
}
