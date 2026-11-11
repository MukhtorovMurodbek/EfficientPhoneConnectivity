use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub clipboard: bool,
    pub notifications: bool,
    /// Bluetooth address of the phone. `None` = first paired phone.
    pub device: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { clipboard: true, notifications: true, device: None }
    }
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("epc").join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read(path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        let res = std::fs::create_dir_all(p.parent().unwrap())
            .and_then(|_| std::fs::write(&p, serde_json::to_vec_pretty(self).unwrap()));
        if let Err(e) = res {
            eprintln!("epcd: saving {}: {e}", p.display());
        }
    }
}
