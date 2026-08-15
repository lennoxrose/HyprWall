use std::collections::HashMap;

use crate::monitor::{Monitor, Rect};

#[derive(Debug, Default, Clone)]
pub struct MonitorRegistry {
    monitors: HashMap<String, Monitor>,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, m: Monitor) {
        self.monitors.insert(m.name.clone(), m);
    }

    pub fn remove(&mut self, name: &str) {
        self.monitors.remove(name);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.monitors.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&Monitor> {
        self.monitors.get(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.monitors.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn geometry(&self) -> HashMap<String, Rect> {
        self.monitors.iter().map(|(k, v)| (k.clone(), v.logical)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, x: i32, w: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            logical: Rect { x, y: 0, w, h: 1080 },
        }
    }

    #[test]
    fn insert_then_contains() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("eDP-1", 0, 1920));
        assert!(reg.contains("eDP-1"));
        assert!(!reg.contains("HDMI-A-1"));
    }

    #[test]
    fn remove_drops_it() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("eDP-1", 0, 1920));
        reg.remove("eDP-1");
        assert!(!reg.contains("eDP-1"));
    }

    #[test]
    fn names_are_sorted() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("HDMI-A-1", 1920, 1920));
        reg.insert(mon("eDP-1", 0, 1920));
        assert_eq!(reg.names(), vec!["HDMI-A-1".to_string(), "eDP-1".to_string()]);
    }
}
