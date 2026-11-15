//! Updates: looks for a newer signed release and tells the panel, which offers it.
//! Nothing is installed until the user asks (`install`), so an update never
//! happens behind their back or in the middle of a meeting they did not choose.
//!
//! The endpoint and the public key live in `tauri.conf.json` (`plugins.updater`).
//! On Windows the installer closes the app and starts the new version.

use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Wait after launch, so the app is up before the first check.
const FIRST_CHECK: Duration = Duration::from_secs(20);
/// Time between two checks.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Left in the app's data folder across the restart of an update the user asked for: the new
/// version then also brings the bar's app up to date, as part of the same request.
pub const BAR_FLAG: &str = "update-bar.flag";

pub async fn run(app: AppHandle) {
    tokio::time::sleep(FIRST_CHECK).await;
    loop {
        check(&app).await;
        tokio::time::sleep(CHECK_EVERY).await;
    }
}

/// Records what a check found: the version on offer, and the words for the version line.
fn record(app: &AppHandle, available: Option<String>, note: Option<String>) {
    let state = app.state::<AppState>();
    *state
        .update_available
        .lock()
        .expect("update state poisoned") = available;
    *state.update_note.lock().expect("update note poisoned") = note;
}

/// Looks for a newer release and remembers the answer. Installs nothing.
pub async fn check(app: &AppHandle) {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => {
            tracing::info!(update = "unavailable", %error);
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            tracing::info!(update = "available", version = %update.version);
            let version = update.version.clone();
            record(app, Some(version.clone()), Some(format!("{version} disponible")));
        }
        Ok(None) => {
            tracing::info!(update = "up to date");
            record(app, None, Some("à jour".to_string()));
        }
        Err(error) => {
            // Offline, or GitHub out of reach: say nothing rather than something wrong.
            tracing::info!(update = "check failed", %error);
            record(app, None, None);
        }
    }
}

fn flag_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join(BAR_FLAG))
}

/// Downloads and installs the release on offer, at the user's request. On success the
/// installer closes this app and starts the new one, so this only returns on failure.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Aucune mise à jour à installer".to_string())?;
    tracing::info!(update = "installing", version = %update.version);
    let flag = flag_path(app);
    if let Some(flag) = &flag {
        let _ = std::fs::write(flag, update.version.as_bytes());
    }
    // Leave the bar clean: the installer is about to close the app.
    let state = app.state::<AppState>();
    state.set_shutting_down();
    if let Ok(client) = crate::busybar::BusyBarClient::new(&state.base_url()) {
        let _guard = state.draw_lock.lock().await;
        let _ = client.clear(crate::screens::APP).await;
    }
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        tracing::info!(update = "install failed", %error);
        // Nothing was installed: the app keeps running, so drawing resumes.
        if let Some(flag) = &flag {
            let _ = std::fs::remove_file(flag);
        }
        state.resume();
        return Err(format!("Mise à jour impossible : {error}"));
    }
    Ok(())
}
