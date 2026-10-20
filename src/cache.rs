//! Zwischenspeicher für gerenderte Seiten mit Speicherobergrenze.
//!
//! Schlüssel ist (Seite, Maßstab in Tausendsteln). Überschreitet die Summe der
//! Pixelbytes die Grenze, fliegen die am längsten nicht gezeigten Einträge
//! hinaus. Der zuletzt eingefügte Eintrag bleibt immer (eine einzelne Seite
//! darf größer sein als die Grenze, sonst gäbe es gar kein Bild), ebenso
//! geschützte Einträge (die gerade sichtbaren Seiten): Sonst verdrängten sich
//! zwei große sichtbare Seiten gegenseitig und würden endlos neu gerendert.

use std::collections::HashMap;

/// Maßstab als ganzzahliger Schlüssel (Tausendstel).
pub fn scale_key(scale: f64) -> u32 {
    (scale * 1000.0).round().max(1.0) as u32
}

struct Entry<V> {
    value: V,
    bytes: usize,
    stamp: u64,
}

pub struct Cache<V> {
    limit: usize,
    used: usize,
    clock: u64,
    entries: HashMap<(usize, u32), Entry<V>>,
}

impl<V: Clone> Cache<V> {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            clock: 0,
            entries: HashMap::new(),
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    pub fn insert(&mut self, page: usize, key: u32, value: V, bytes: usize) {
        self.insert_protected(page, key, value, bytes, |_| false);
    }

    /// Wie [`Self::insert`]; Einträge, für die `protected` wahr ist, bleiben.
    pub fn insert_protected(
        &mut self,
        page: usize,
        key: u32,
        value: V,
        bytes: usize,
        protected: impl Fn(&(usize, u32)) -> bool,
    ) {
        let stamp = self.tick();
        if let Some(old) = self.entries.insert(
            (page, key),
            Entry {
                value,
                bytes,
                stamp,
            },
        ) {
            self.used -= old.bytes;
        }
        self.used += bytes;
        let keep = (page, key);
        self.evict(|k| *k == keep || protected(k));
    }

    fn evict(&mut self, keep: impl Fn(&(usize, u32)) -> bool) {
        while self.used > self.limit {
            let oldest = self
                .entries
                .iter()
                .filter(|(k, _)| !keep(k))
                .min_by_key(|(_, e)| e.stamp)
                .map(|(k, _)| *k);
            let Some(k) = oldest else { break };
            if let Some(e) = self.entries.remove(&k) {
                self.used -= e.bytes;
            }
        }
    }

    /// Exakter Eintrag; zählt als „gezeigt“.
    pub fn get(&mut self, page: usize, key: u32) -> Option<V> {
        let stamp = self.tick();
        self.entries.get_mut(&(page, key)).map(|e| {
            e.stamp = stamp;
            e.value.clone()
        })
    }

    pub fn contains(&self, page: usize, key: u32) -> bool {
        self.entries.contains_key(&(page, key))
    }

    /// Bester vorhandener Eintrag für `page`: exakt, sonst der mit dem
    /// nächstliegenden Maßstab (als Platzhalter, bis die Seite neu gerendert
    /// ist). Gibt den Maßstab des Eintrags mit zurück.
    pub fn best(&mut self, page: usize, key: u32) -> Option<(V, u32)> {
        if let Some(v) = self.get(page, key) {
            return Some((v, key));
        }
        let found = self
            .entries
            .keys()
            .filter(|(p, _)| *p == page)
            .min_by_key(|(_, k)| k.abs_diff(key))
            .copied()?;
        self.get(found.0, found.1).map(|v| (v, found.1))
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.used = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_to_thousandths() {
        assert_eq!(scale_key(1.0), 1000);
        assert_eq!(scale_key(1.33333), 1333);
        assert_eq!(scale_key(0.0), 1);
    }

    #[test]
    fn least_recently_shown_entries_are_evicted_first() {
        let mut c = Cache::new(300);
        c.insert(0, 1000, "a", 100);
        c.insert(1, 1000, "b", 100);
        c.insert(2, 1000, "c", 100);
        assert_eq!(c.used(), 300);
        // a wird gezeigt, also ist b am ältesten
        assert_eq!(c.get(0, 1000), Some("a"));
        c.insert(3, 1000, "d", 100);
        assert!(!c.contains(1, 1000));
        assert!(c.contains(0, 1000) && c.contains(2, 1000) && c.contains(3, 1000));
        assert_eq!(c.used(), 300);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn the_newest_entry_stays_even_above_the_limit() {
        let mut c = Cache::new(100);
        c.insert(0, 1000, "klein", 50);
        c.insert(1, 4000, "riesig", 500);
        assert!(c.contains(1, 4000));
        assert!(!c.contains(0, 1000));
        assert_eq!(c.used(), 500);
        // das nächste verdrängt das riesige
        c.insert(2, 1000, "x", 10);
        assert!(!c.contains(1, 4000));
        assert_eq!(c.used(), 10);
    }

    #[test]
    fn protected_entries_survive_eviction() {
        let mut c = Cache::new(100);
        c.insert(0, 1000, "sichtbar", 80);
        c.insert_protected(1, 1000, "auch sichtbar", 80, |k| k.0 == 0);
        assert!(c.contains(0, 1000) && c.contains(1, 1000));
        assert_eq!(c.used(), 160);
        // ohne Schutz fällt der älteste wieder heraus, bis die Grenze passt
        c.insert(2, 1000, "neu", 10);
        assert!(!c.contains(0, 1000));
        assert!(c.contains(1, 1000) && c.contains(2, 1000));
        assert_eq!(c.used(), 90);
    }

    #[test]
    fn replacing_an_entry_keeps_the_byte_count_right() {
        let mut c = Cache::new(1000);
        c.insert(0, 1000, 1, 100);
        c.insert(0, 1000, 2, 300);
        assert_eq!(c.used(), 300);
        assert_eq!(c.len(), 1);
        assert_eq!(c.get(0, 1000), Some(2));
        c.clear();
        assert_eq!((c.used(), c.len()), (0, 0));
    }

    #[test]
    fn best_falls_back_to_the_nearest_scale() {
        let mut c = Cache::new(1000);
        c.insert(5, 500, "halb", 10);
        c.insert(5, 2000, "doppelt", 10);
        c.insert(6, 1000, "andere Seite", 10);
        assert_eq!(c.best(5, 1000), Some(("halb", 500)));
        assert_eq!(c.best(5, 1800), Some(("doppelt", 2000)));
        assert_eq!(c.best(5, 2000), Some(("doppelt", 2000)));
        assert_eq!(c.best(7, 1000), None);
    }
}
