//! Automatic updates: looks for a newer signed release and installs it when no
//! meeting is on screen, so an update never interrupts a meeting.
//!
//! The endpoint and the public key live in `tauri.conf.json` (`plugins.updater`).
//! On Windows the installer closes the app and starts the new version.

use crate::meeting_state::MeetingState;
use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

/// Wait after launch, so the app is up before the first check.
const FIRST_CHECK: Duration = Duration::from_secs(60);
/// Time between two checks.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// When an update is ready but a meeting is in progress, look again this often.
const RETRY_WHEN_BUSY: Duration = Duration::from_secs(5 * 60);

pub async fn run(app: AppHandle) {
    tokio::time::sleep(FIRST_CHECK).await;
    loop {
        let wait = match check_and_install(&app).await {
            Outcome::Busy => RETRY_WHEN_BUSY,
            Outcome::Done => CHECK_EVERY,
        };
        tokio::time::sleep(wait).await;
    }
}

enum Outcome {
    /// An update exists but a meeting is on screen: try again soon.
    Busy,
    /// Nothing to do, or the check failed: wait for the next round.
    Done,
}

fn meeting_in_progress(app: &AppHandle) -> bool {
    !matches!(
        *app.state::<AppState>()
            .meeting_state
            .lock()
            .expect("meeting state poisoned"),
        MeetingState::Idle
    )
}

async fn check_and_install(app: &AppHandle) -> Outcome {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => {
            tracing::info!(update = "unavailable", %error);
            return Outcome::Done;
        }
    };
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            tracing::info!(update = "up to date");
            return Outcome::Done;
        }
        Err(error) => {
            tracing::info!(update = "check failed", %error);
            return Outcome::Done;
        }
    };
    if meeting_in_progress(app) {
        tracing::info!(update = "postponed", version = %update.version);
        return Outcome::Busy;
    }
    tracing::info!(update = "installing", version = %update.version);
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
        state.resume();
    }
    Outcome::Done
}
