use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::commands::DEFAULT_BASE_URL;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub base_url: String,
    #[serde(default)]
    pub ics_url: String,
    #[serde(default = "default_policy")]
    pub busy_session_policy: String,
    #[serde(default)]
    pub trigger_smart_home: bool,
    #[serde(default)]
    pub quiet_hours_start: String,
    #[serde(default)]
    pub quiet_hours_end: String,
    #[serde(default)]
    pub discreet_mode: bool,
    /// Switch to the "in meeting" screen at the start time without waiting
    /// for the user to press Start.
    #[serde(default)]
    pub auto_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            ics_url: String::new(),
            busy_session_policy: default_policy(),
            trigger_smart_home: false,
            quiet_hours_start: String::new(),
            quiet_hours_end: String::new(),
            discreet_mode: false,
            auto_start: false,
        }
    }
}

fn default_policy() -> String {
    "interrupt".to_string()
}

pub fn load<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<Settings, String> {
    let store = app
        .store("settings.json")
        .map_err(|error| error.to_string())?;
    let base_url = store
        .get("base_url")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let ics_url = store
        .get("ics_url")
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    let busy_session_policy = store
        .get("busy_session_policy")
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(default_policy);
    let trigger_smart_home = store
        .get("trigger_smart_home")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let quiet_hours_start = store
        .get("quiet_hours_start")
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    let quiet_hours_end = store
        .get("quiet_hours_end")
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default();
    let discreet_mode = store
        .get("discreet_mode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let auto_start = store
        .get("auto_start")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(Settings {
        base_url,
        ics_url,
        busy_session_policy,
        trigger_smart_home,
        quiet_hours_start,
        quiet_hours_end,
        discreet_mode,
        auto_start,
    })
}

pub fn save<R: tauri::Runtime>(app: &AppHandle<R>, settings: &Settings) -> Result<(), String> {
    let store = app
        .store("settings.json")
        .map_err(|error| error.to_string())?;
    store.set("base_url", serde_json::json!(settings.base_url.clone()));
    store.set("ics_url", serde_json::json!(settings.ics_url.clone()));
    store.set(
        "busy_session_policy",
        serde_json::json!(settings.busy_session_policy.clone()),
    );
    store.set(
        "trigger_smart_home",
        serde_json::json!(settings.trigger_smart_home),
    );
    store.set(
        "quiet_hours_start",
        serde_json::json!(settings.quiet_hours_start.clone()),
    );
    store.set(
        "quiet_hours_end",
        serde_json::json!(settings.quiet_hours_end.clone()),
    );
    store.set("discreet_mode", serde_json::json!(settings.discreet_mode));
    store.set("auto_start", serde_json::json!(settings.auto_start));
    store.save().map_err(|error| error.to_string())
}
