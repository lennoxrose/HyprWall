use std::fmt;

use crate::monitor::Rect;
use crate::monitor_registry::MonitorRegistry;
use crate::zone::Zone;

#[derive(Debug, Default)]
pub struct ZoneManager {
    zones: Vec<Zone>,
    next_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneApplyOutcome {
    pub zone_id: u64,
    pub bounding_box: Rect,
    pub dissolved_zone_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneError {
    UnknownMonitor(String),
}

impl fmt::Display for ZoneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneError::UnknownMonitor(name) => write!(f, "unknown monitor {name}"),
        }
    }
}

impl ZoneManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Groups `monitors` into one zone playing `path`. Any named monitor
    /// already in a different zone is pulled out of it first; a zone left
    /// with no monitors is dissolved. Returns the id of the (re)formed zone
    /// and the ids of any zones dissolved as a side effect.
    pub fn apply_set(
        &mut self,
        monitors: &[String],
        path: String,
        registry: &MonitorRegistry,
    ) -> Result<ZoneApplyOutcome, ZoneError> {
        for name in monitors {
            if !registry.contains(name) {
                return Err(ZoneError::UnknownMonitor(name.clone()));
            }
        }

        let mut dissolved_zone_ids = Vec::new();

        // Pull each named monitor out of whatever zone currently holds it.
        for name in monitors {
            for zone in &mut self.zones {
                zone.monitors.retain(|m| m != name);
            }
        }
        // Dissolve now-empty zones (other than the one we're about to (re)form).
        self.zones.retain(|z| {
            if z.monitors.is_empty() {
                dissolved_zone_ids.push(z.id);
                false
            } else {
                true
            }
        });

        let geometry = registry.geometry();
        let rects: Vec<Rect> = monitors.iter().map(|m| geometry[m]).collect();
        let bounding_box = Rect::union(&rects).expect("monitors is non-empty");

        // Reuse an existing zone id if this exact monitor set already forms one
        // (e.g. re-`set` with a new path); otherwise mint a new zone.
        let mut sorted_new = monitors.to_vec();
        sorted_new.sort();
        let existing = self.zones.iter_mut().find(|z| {
            let mut sorted_existing = z.monitors.clone();
            sorted_existing.sort();
            sorted_existing == sorted_new
        });

        let zone_id = if let Some(zone) = existing {
            zone.path = Some(path);
            zone.bounding_box = Some(bounding_box);
            zone.id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.zones.push(Zone {
                id,
                monitors: monitors.to_vec(),
                path: Some(path),
                bounding_box: Some(bounding_box),
            });
            id
        };

        Ok(ZoneApplyOutcome { zone_id, bounding_box, dissolved_zone_ids })
    }

    pub fn zone_for_monitor(&self, monitor: &str) -> Option<&Zone> {
        self.zones.iter().find(|z| z.monitors.iter().any(|m| m == monitor))
    }

    pub fn path_for_monitor(&self, monitor: &str) -> Option<&str> {
        self.zone_for_monitor(monitor)?.path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Monitor;

    fn registry_with(names: &[&str]) -> MonitorRegistry {
        let mut reg = MonitorRegistry::new();
        for (i, name) in names.iter().enumerate() {
            reg.insert(Monitor {
                name: name.to_string(),
                logical: Rect { x: (i as i32) * 1920, y: 0, w: 1920, h: 1080 },
            });
        }
        reg
    }

    #[test]
    fn set_single_monitor_forms_zone_of_one() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm.apply_set(&["eDP-1".to_string()], "/a.mp4".to_string(), &reg).unwrap();
        assert_eq!(outcome.bounding_box, Rect { x: 0, y: 0, w: 1920, h: 1080 });
        assert!(outcome.dissolved_zone_ids.is_empty());
        assert_eq!(zm.path_for_monitor("eDP-1"), Some("/a.mp4"));
    }

    #[test]
    fn set_two_monitors_spans_bounding_box() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm
            .apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();
        assert_eq!(outcome.bounding_box, Rect { x: 0, y: 0, w: 3840, h: 1080 });
        assert_eq!(zm.zone_for_monitor("eDP-1").unwrap().id, zm.zone_for_monitor("HDMI-A-1").unwrap().id);
    }

    #[test]
    fn re_setting_one_monitor_splits_it_out_of_a_zone() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();

        let outcome = zm.apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg).unwrap();

        assert_eq!(zm.path_for_monitor("eDP-1"), Some("/solo.mp4"));
        // The old two-monitor zone had HDMI-A-1 left in it after eDP-1 was pulled out,
        // so it survives (not dissolved) with just HDMI-A-1, still on the old path.
        assert_eq!(zm.path_for_monitor("HDMI-A-1"), Some("/pano.mp4"));
        assert!(outcome.dissolved_zone_ids.is_empty());
    }

    #[test]
    fn re_setting_both_members_alone_dissolves_the_zone() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        let outcome = zm.apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg).unwrap();
        assert!(outcome.dissolved_zone_ids.is_empty(), "HDMI-A-1 still holds the old zone open");

        let outcome2 = zm.apply_set(&["HDMI-A-1".to_string()], "/other.mp4".to_string(), &reg).unwrap();
        assert_eq!(outcome2.dissolved_zone_ids, vec![zone_id]);
    }

    #[test]
    fn unknown_monitor_is_rejected() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let err = zm.apply_set(&["eDP-9".to_string()], "/a.mp4".to_string(), &reg).unwrap_err();
        assert_eq!(err, ZoneError::UnknownMonitor("eDP-9".to_string()));
    }
}
