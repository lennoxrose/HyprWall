use std::path::PathBuf;

use crate::monitor_registry::MonitorRegistry;
use crate::zone_manager::ZoneManager;

pub struct AppState {
    pub registry: MonitorRegistry,
    pub zones: ZoneManager,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn new(registry: MonitorRegistry, config_path: PathBuf) -> Self {
        Self { registry, zones: ZoneManager::new(), config_path }
    }
}
