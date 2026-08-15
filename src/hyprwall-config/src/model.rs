use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub zones: Vec<ZoneConfig>,
    #[serde(default)]
    pub library_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneConfig {
    pub monitors: Vec<String>,
    pub path: String,
}
