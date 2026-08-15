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

/// Result of `clear_path`, telling the caller exactly what happened to
/// `monitor`'s zone so it knows which `RenderResources` teardown to run (if
/// any) and whether to bother persisting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearOutcome {
    /// `monitor` isn't in any zone at all.
    NotFound,
    /// The zone's path was already `None` (e.g. the second member of a
    /// group being unset in the same batch) -- nothing to do.
    AlreadyCleared,
    /// A solo (single-monitor) zone had nothing worth keeping once its path
    /// is gone, so it was dissolved same as before.
    Dissolved { zone_id: u64 },
    /// A real (multi-monitor) zone kept every member -- only its path was
    /// cleared. Playback should stop for all of `monitors`, but the group
    /// itself survives so a later `Set` on the same members reforms it.
    Cleared { zone_id: u64, monitors: Vec<String> },
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

        Ok(ZoneApplyOutcome {
            zone_id,
            bounding_box,
            dissolved_zone_ids,
        })
    }

    /// Removes `monitor` from whatever zone currently holds it (if any),
    /// dissolving that zone if it becomes empty as a result. Used for a
    /// monitor unplug (`output_destroyed`), not a `Set` -- unlike
    /// `apply_set`, this never tries to keep a zone alive for monitors that
    /// were never named; it only reflects one monitor's disappearance.
    ///
    /// Returns the id of the zone that was dissolved, if any. A zone that
    /// still has other monitors after `monitor` is removed keeps playing to
    /// them and is *not* returned/dissolved (matches the spec: unplugging one
    /// member of a multi-monitor zone keeps the zone running for the rest).
    pub fn remove_monitor(&mut self, monitor: &str) -> Option<u64> {
        for zone in &mut self.zones {
            zone.monitors.retain(|m| m != monitor);
        }
        let mut dissolved = None;
        self.zones.retain(|z| {
            if z.monitors.is_empty() {
                dissolved = Some(z.id);
                false
            } else {
                true
            }
        });
        dissolved
    }

    /// Clears the wallpaper for `monitor`'s zone -- what a user-initiated
    /// "remove wallpaper" means, as opposed to `remove_monitor`'s "this
    /// monitor is physically gone". A solo zone has nothing left worth
    /// keeping once its path is gone, so it's dissolved exactly like before.
    /// A real (multi-monitor) zone instead keeps every member and just loses
    /// its path -- the group survives with no wallpaper, ready for a later
    /// `Set` on the same members to reform it, rather than splitting apart.
    pub fn clear_path(&mut self, monitor: &str) -> ClearOutcome {
        let Some(zone) = self.zones.iter_mut().find(|z| z.monitors.iter().any(|m| m == monitor)) else {
            return ClearOutcome::NotFound;
        };
        if zone.path.is_none() {
            return ClearOutcome::AlreadyCleared;
        }
        if zone.monitors.len() == 1 {
            let zone_id = zone.id;
            self.zones.retain(|z| z.id != zone_id);
            return ClearOutcome::Dissolved { zone_id };
        }
        zone.path = None;
        ClearOutcome::Cleared {
            zone_id: zone.id,
            monitors: zone.monitors.clone(),
        }
    }

    pub fn zone_for_monitor(&self, monitor: &str) -> Option<&Zone> {
        self.zones.iter().find(|z| z.monitors.iter().any(|m| m == monitor))
    }

    pub fn path_for_monitor(&self, monitor: &str) -> Option<&str> {
        self.zone_for_monitor(monitor)?.path.as_deref()
    }

    /// Every zone id currently assigned `path`, in no particular order.
    /// Used by `Command::SetWallpaperSettings` to find which live
    /// `MpvInstance`s a settings change should reach immediately.
    pub fn zone_ids_with_path(&self, path: &str) -> Vec<u64> {
        self.zones
            .iter()
            .filter(|z| z.path.as_deref() == Some(path))
            .map(|z| z.id)
            .collect()
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
                logical: Rect {
                    x: (i as i32) * 1920,
                    y: 0,
                    w: 1920,
                    h: 1080,
                },
            });
        }
        reg
    }

    #[test]
    fn zone_ids_with_path_finds_a_single_match() {
        let mut zm = ZoneManager::new();
        let registry = registry_with(&["eDP-1"]);
        zm.apply_set(&["eDP-1".to_string()], "/a.jpg".to_string(), &registry)
            .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        assert_eq!(zm.zone_ids_with_path("/a.jpg"), vec![zone_id]);
    }

    #[test]
    fn zone_ids_with_path_is_empty_when_nothing_matches() {
        let mut zm = ZoneManager::new();
        let registry = registry_with(&["eDP-1"]);
        zm.apply_set(&["eDP-1".to_string()], "/a.jpg".to_string(), &registry)
            .unwrap();

        assert_eq!(zm.zone_ids_with_path("/other.jpg"), Vec::<u64>::new());
    }

    #[test]
    fn zone_ids_with_path_only_returns_matching_zones() {
        let mut zm = ZoneManager::new();
        let registry = registry_with(&["eDP-1", "HDMI-A-1"]);
        zm.apply_set(&["eDP-1".to_string()], "/a.jpg".to_string(), &registry)
            .unwrap();
        zm.apply_set(&["HDMI-A-1".to_string()], "/b.jpg".to_string(), &registry)
            .unwrap();
        let a_zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        assert_eq!(zm.zone_ids_with_path("/a.jpg"), vec![a_zone_id]);
    }

    #[test]
    fn set_single_monitor_forms_zone_of_one() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm
            .apply_set(&["eDP-1".to_string()], "/a.mp4".to_string(), &reg)
            .unwrap();
        assert_eq!(
            outcome.bounding_box,
            Rect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080
            }
        );
        assert!(outcome.dissolved_zone_ids.is_empty());
        assert_eq!(zm.path_for_monitor("eDP-1"), Some("/a.mp4"));
    }

    #[test]
    fn set_two_monitors_spans_bounding_box() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm
            .apply_set(
                &["eDP-1".to_string(), "HDMI-A-1".to_string()],
                "/pano.mp4".to_string(),
                &reg,
            )
            .unwrap();
        assert_eq!(
            outcome.bounding_box,
            Rect {
                x: 0,
                y: 0,
                w: 3840,
                h: 1080
            }
        );
        assert_eq!(
            zm.zone_for_monitor("eDP-1").unwrap().id,
            zm.zone_for_monitor("HDMI-A-1").unwrap().id
        );
    }

    #[test]
    fn re_setting_one_monitor_splits_it_out_of_a_zone() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(
            &["eDP-1".to_string(), "HDMI-A-1".to_string()],
            "/pano.mp4".to_string(),
            &reg,
        )
        .unwrap();

        let outcome = zm
            .apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg)
            .unwrap();

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
        zm.apply_set(
            &["eDP-1".to_string(), "HDMI-A-1".to_string()],
            "/pano.mp4".to_string(),
            &reg,
        )
        .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        let outcome = zm
            .apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg)
            .unwrap();
        assert!(
            outcome.dissolved_zone_ids.is_empty(),
            "HDMI-A-1 still holds the old zone open"
        );

        let outcome2 = zm
            .apply_set(&["HDMI-A-1".to_string()], "/other.mp4".to_string(), &reg)
            .unwrap();
        assert_eq!(outcome2.dissolved_zone_ids, vec![zone_id]);
    }

    #[test]
    fn unknown_monitor_is_rejected() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let err = zm
            .apply_set(&["eDP-9".to_string()], "/a.mp4".to_string(), &reg)
            .unwrap_err();
        assert_eq!(err, ZoneError::UnknownMonitor("eDP-9".to_string()));
    }

    #[test]
    fn remove_monitor_dissolves_a_single_monitor_zone() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string()], "/a.mp4".to_string(), &reg)
            .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        let dissolved = zm.remove_monitor("eDP-1");

        assert_eq!(dissolved, Some(zone_id));
        assert!(zm.zone_for_monitor("eDP-1").is_none());
    }

    #[test]
    fn remove_monitor_keeps_a_multi_monitor_zone_alive_for_the_rest() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(
            &["eDP-1".to_string(), "HDMI-A-1".to_string()],
            "/pano.mp4".to_string(),
            &reg,
        )
        .unwrap();

        let dissolved = zm.remove_monitor("eDP-1");

        assert_eq!(dissolved, None, "HDMI-A-1 is still in the zone, it should not dissolve");
        assert_eq!(zm.path_for_monitor("HDMI-A-1"), Some("/pano.mp4"));
        assert!(zm.zone_for_monitor("eDP-1").is_none());
    }

    #[test]
    fn remove_monitor_not_in_any_zone_is_a_no_op() {
        let mut zm = ZoneManager::new();
        assert_eq!(zm.remove_monitor("eDP-1"), None);
    }

    #[test]
    fn clear_path_on_unknown_monitor_reports_not_found() {
        let mut zm = ZoneManager::new();
        assert_eq!(zm.clear_path("eDP-1"), ClearOutcome::NotFound);
    }

    #[test]
    fn clear_path_dissolves_a_solo_zone() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string()], "/a.mp4".to_string(), &reg)
            .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        assert_eq!(zm.clear_path("eDP-1"), ClearOutcome::Dissolved { zone_id });
        assert!(zm.zone_for_monitor("eDP-1").is_none());
    }

    #[test]
    fn clear_path_on_a_group_keeps_every_member_but_drops_the_path() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(
            &["eDP-1".to_string(), "HDMI-A-1".to_string()],
            "/pano.mp4".to_string(),
            &reg,
        )
        .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        let outcome = zm.clear_path("eDP-1");
        assert_eq!(
            outcome,
            ClearOutcome::Cleared {
                zone_id,
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()]
            }
        );
        assert!(zm.path_for_monitor("eDP-1").is_none());
        assert!(zm.path_for_monitor("HDMI-A-1").is_none());
        assert_eq!(zm.zone_for_monitor("eDP-1").unwrap().id, zone_id);
        assert_eq!(
            zm.zone_for_monitor("HDMI-A-1").unwrap().id,
            zone_id,
            "group must survive intact"
        );
    }

    #[test]
    fn clear_path_twice_in_a_row_is_idempotent() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(
            &["eDP-1".to_string(), "HDMI-A-1".to_string()],
            "/pano.mp4".to_string(),
            &reg,
        )
        .unwrap();

        zm.clear_path("eDP-1");
        assert_eq!(zm.clear_path("HDMI-A-1"), ClearOutcome::AlreadyCleared);
    }
}
