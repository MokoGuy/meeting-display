use crate::busybar::{BusyBarClient, InputKey};
use crate::state::{AppState, ConnectionDto};
use chrono::Utc;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

pub const DEFAULT_BASE_URL: &str = "http://10.0.4.20";

#[tauri::command]
pub async fn send_start(app: AppHandle) -> Result<(), String> {
    if matches!(
        *app.state::<AppState>()
            .meeting_state
            .lock()
            .expect("meeting state poisoned"),
        crate::meeting_state::MeetingState::Armed { .. }
    ) {
        return confirm_meeting_start(app).await;
    }
    Err("No armed meeting".to_string())
}

#[tauri::command]
pub fn save_meeting_settings(
    app: AppHandle,
    policy: String,
    trigger_smart_home: bool,
    quiet_hours_start: String,
    quiet_hours_end: String,
    discreet_mode: bool,
    auto_start: bool,
) -> Result<(), String> {
    if !matches!(
        policy.as_str(),
        "interrupt" | "toast_then_interrupt" | "do_not_disturb"
    ) {
        return Err("Invalid busy session policy".to_string());
    }
    let current = crate::settings::Settings {
        base_url: app.state::<AppState>().base_url(),
        ics_url: app.state::<AppState>().ics_url(),
        busy_session_policy: policy.clone(),
        trigger_smart_home,
        quiet_hours_start: quiet_hours_start.clone(),
        quiet_hours_end: quiet_hours_end.clone(),
        discreet_mode,
        auto_start,
    };
    crate::settings::save(&app, &current)?;
    *app.state::<AppState>()
        .busy_session_policy
        .lock()
        .expect("policy state poisoned") = policy;
    *app.state::<AppState>()
        .trigger_smart_home
        .lock()
        .expect("smart home state poisoned") = trigger_smart_home;
    *app.state::<AppState>()
        .quiet_hours_start
        .lock()
        .expect("quiet hours state poisoned") = quiet_hours_start;
    *app.state::<AppState>()
        .quiet_hours_end
        .lock()
        .expect("quiet hours state poisoned") = quiet_hours_end;
    *app.state::<AppState>()
        .discreet_mode
        .lock()
        .expect("discreet state poisoned") = discreet_mode;
    *app.state::<AppState>()
        .auto_start
        .lock()
        .expect("auto start state poisoned") = auto_start;
    Ok(())
}

/// What the popover shows about the current or next meeting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MeetingStatusDto {
    /// "none", "upcoming", "armed", "live" or "overrun".
    pub phase: String,
    pub meeting: Option<crate::calendar::Meeting>,
    /// Meetings set aside by the user: id to "done" or "skipped".
    pub dismissed: std::collections::HashMap<String, String>,
}

#[tauri::command]
pub fn get_meeting_status(app: AppHandle) -> MeetingStatusDto {
    use crate::meeting_state::MeetingState;
    let state = app.state::<AppState>();
    let current = state
        .meeting_state
        .lock()
        .expect("meeting state poisoned")
        .clone();
    let (phase, meeting) = match current {
        MeetingState::Armed { meeting } => ("armed", Some(meeting)),
        MeetingState::InMeeting { meeting, .. } => ("live", Some(meeting)),
        MeetingState::Overrun { meeting, .. } => ("overrun", Some(meeting)),
        MeetingState::Idle => {
            let now = Utc::now();
            let next = state
                .visible_meetings()
                .into_iter()
                .filter(|meeting| meeting.start > now)
                .min_by_key(|meeting| meeting.start);
            (if next.is_some() { "upcoming" } else { "none" }, next)
        }
    };
    MeetingStatusDto {
        phase: phase.to_string(),
        meeting,
        dismissed: state.dismissed_map(),
    }
}

#[tauri::command]
pub async fn send_key(app: AppHandle, key: String) -> Result<(), String> {
    let input = match InputKey::parse(&key) {
        Ok(input) => input,
        Err(error) => {
            notify_failure(&app, &error);
            return Err(error);
        }
    };
    send_input(app, input).await
}

async fn send_input(app: AppHandle, key: InputKey) -> Result<(), String> {
    let client = match BusyBarClient::new(&app.state::<AppState>().base_url()) {
        Ok(client) => client,
        Err(error) => {
            let message = error.to_string();
            notify_failure(&app, &message);
            return Err(message);
        }
    };
    match client.input(key).await {
        Ok(()) => {
            notify(&app, "Touche envoyée")?;
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            notify_failure(&app, &message);
            Err(message)
        }
    }
}

#[tauri::command]
pub fn get_connection(app: AppHandle) -> ConnectionDto {
    app.state::<AppState>().dto()
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<crate::settings::Settings, String> {
    Ok(crate::settings::Settings {
        base_url: app.state::<AppState>().base_url(),
        ics_url: app.state::<AppState>().ics_url(),
        busy_session_policy: app
            .state::<AppState>()
            .busy_session_policy
            .lock()
            .expect("policy state poisoned")
            .clone(),
        trigger_smart_home: *app
            .state::<AppState>()
            .trigger_smart_home
            .lock()
            .expect("smart home state poisoned"),
        quiet_hours_start: app
            .state::<AppState>()
            .quiet_hours_start
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        quiet_hours_end: app
            .state::<AppState>()
            .quiet_hours_end
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        auto_start: app.state::<AppState>().auto_start(),
        discreet_mode: *app
            .state::<AppState>()
            .discreet_mode
            .lock()
            .expect("discreet state poisoned"),
    })
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, base_url: String) -> Result<(), String> {
    BusyBarClient::new(&base_url).map_err(|error| error.to_string())?;
    let settings = crate::settings::Settings {
        base_url,
        ics_url: app.state::<AppState>().ics_url(),
        busy_session_policy: app
            .state::<AppState>()
            .busy_session_policy
            .lock()
            .expect("policy state poisoned")
            .clone(),
        trigger_smart_home: *app
            .state::<AppState>()
            .trigger_smart_home
            .lock()
            .expect("smart home state poisoned"),
        quiet_hours_start: app
            .state::<AppState>()
            .quiet_hours_start
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        quiet_hours_end: app
            .state::<AppState>()
            .quiet_hours_end
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        auto_start: app.state::<AppState>().auto_start(),
        discreet_mode: *app
            .state::<AppState>()
            .discreet_mode
            .lock()
            .expect("discreet state poisoned"),
    };
    crate::settings::save(&app, &settings)?;
    app.state::<AppState>().set_base_url(settings.base_url);
    Ok(())
}

/// The user confirms that the armed meeting has started: switch to the
/// "in meeting" screen (time left until the scheduled end) right away.
#[tauri::command]
pub async fn confirm_meeting_start(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut current = state.meeting_state.lock().expect("meeting state poisoned");
        let meeting = match &*current {
            crate::meeting_state::MeetingState::Armed { meeting } => meeting.clone(),
            _ => return Err("No armed meeting".to_string()),
        };
        *current = crate::meeting_state::MeetingState::InMeeting {
            end: meeting.end,
            meeting,
            saved_snapshot: None,
            started_at: Utc::now(),
        };
    }
    crate::scheduler::tick(&app).await;
    Ok(())
}

/// Goes live now: for the armed meeting, or ahead of time for the next one.
#[tauri::command]
pub async fn start_meeting_now(app: AppHandle) -> Result<(), String> {
    use crate::meeting_state::MeetingState;
    {
        let state = app.state::<AppState>();
        let now = Utc::now();
        let current = state
            .meeting_state
            .lock()
            .expect("meeting state poisoned")
            .clone();
        let (meeting, was_armed) = match current {
            MeetingState::Armed { meeting } => (meeting, true),
            MeetingState::InMeeting { .. } => return Err("Already in a meeting".to_string()),
            MeetingState::Idle | MeetingState::Overrun { .. } => {
                let next = state
                    .visible_meetings()
                    .into_iter()
                    .filter(|meeting| meeting.end > now)
                    .min_by_key(|meeting| meeting.start);
                match next {
                    Some(meeting) => (meeting, false),
                    None => return Err("No meeting to start".to_string()),
                }
            }
        };
        // Started ahead of time: the planned length runs from now, not until the scheduled end.
        let meeting = crate::meeting_state::started_now(meeting, now);
        *state.meeting_state.lock().expect("meeting state poisoned") = MeetingState::InMeeting {
            end: meeting.end,
            meeting,
            saved_snapshot: None,
            started_at: now,
        };
        // The armed screen already played the start chime.
        if !was_armed {
            crate::scheduler::play_audibly(state.base_url(), crate::screens::MEETING_START_SOUND);
        }
    }
    crate::scheduler::tick(&app).await;
    Ok(())
}

/// Ignores a meeting: the armed one, or else the next upcoming one.
#[tauri::command]
pub async fn skip_meeting(app: AppHandle) -> Result<(), String> {
    use crate::meeting_state::MeetingState;
    {
        let state = app.state::<AppState>();
        let now = Utc::now();
        let mut current = state.meeting_state.lock().expect("meeting state poisoned");
        match &*current {
            MeetingState::Armed { meeting } => {
                state.record_skipped(meeting);
                *current = MeetingState::Idle;
            }
            MeetingState::Idle => {
                let next = state
                    .visible_meetings()
                    .into_iter()
                    .filter(|meeting| meeting.start > now)
                    .min_by_key(|meeting| meeting.start);
                match next {
                    Some(meeting) => state.record_skipped(&meeting),
                    None => return Err("No meeting to skip".to_string()),
                }
            }
            _ => return Err("A meeting is in progress".to_string()),
        }
    }
    crate::scheduler::tick(&app).await;
    Ok(())
}

/// Ends the meeting in progress (or its overrun) right away, without the end chime.
#[tauri::command]
pub async fn end_meeting(app: AppHandle) -> Result<(), String> {
    use crate::meeting_state::MeetingState;
    {
        let state = app.state::<AppState>();
        let mut current = state.meeting_state.lock().expect("meeting state poisoned");
        let now = Utc::now();
        match &*current {
            // Ended by hand: the real length runs from the start to now.
            MeetingState::InMeeting {
                meeting,
                started_at,
                ..
            } => state.record_done(meeting, *started_at, now),
            // Already recorded when it reached its scheduled end: only the end moves.
            MeetingState::Overrun { meeting, .. } => state.close_done(&meeting.id, now),
            _ => return Err("No meeting in progress".to_string()),
        }
        *current = MeetingState::Idle;
    }
    crate::scheduler::tick(&app).await;
    Ok(())
}

/// Puts one meeting the user skipped or ended back in play.
#[tauri::command]
pub async fn resume_meeting(app: AppHandle, id: String) -> Result<(), String> {
    app.state::<AppState>().resume_meeting(&id);
    crate::scheduler::tick(&app).await;
    Ok(())
}

/// What is installed and what could be, for the version line and the update banner.
#[derive(serde::Serialize)]
pub struct VersionsDto {
    /// This app.
    app: String,
    /// What the last update check found ("à jour", "0.5.6 disponible"), if one has run.
    update: Option<String>,
    /// The newer release on offer, if any.
    available: Option<String>,
    /// The on-bar companion app: its version, or `None` when it is not installed.
    bar_app: Option<String>,
    /// Whether the bar answered at all; the two bar fields mean nothing otherwise.
    bar_reachable: bool,
    /// The bar's app is missing or older than the one this app carries.
    bar_behind: bool,
}

#[tauri::command]
pub async fn get_versions(app: AppHandle) -> Result<VersionsDto, String> {
    let state = app.state::<AppState>();
    let update = state
        .update_note
        .lock()
        .expect("update note poisoned")
        .clone();
    let available = state
        .update_available
        .lock()
        .expect("update state poisoned")
        .clone();
    // The bar keeps each installed app's manifest; reading it works whether the app runs or not.
    let on_bar = match BusyBarClient::new(&state.base_url()) {
        Ok(client) => crate::companion::bar_app_version(&client).await.ok(),
        Err(_) => None,
    };
    let bar_reachable = on_bar.is_some();
    let bar_app = on_bar.flatten();
    Ok(VersionsDto {
        app: app.package_info().version.to_string(),
        update,
        available,
        bar_behind: bar_reachable && crate::companion::bar_app_behind(bar_app.as_deref()),
        bar_app,
        bar_reachable,
    })
}

/// The user's "update" click: this app first when a newer release is on offer (it restarts,
/// and the new version then updates the bar), otherwise the bar's app alone.
#[tauri::command]
pub async fn apply_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let available = state
        .update_available
        .lock()
        .expect("update state poisoned")
        .is_some();
    if available {
        return crate::updater::install(&app).await;
    }
    let client = BusyBarClient::new(&state.base_url()).map_err(|error| error.to_string())?;
    crate::companion::install_bar_app(&client).await
}

/// The local history: meetings held or skipped over the last 30 days.
#[tauri::command]
pub fn get_history(app: AppHandle) -> Vec<crate::history::Entry> {
    app.state::<AppState>().history_entries()
}

#[tauri::command]
pub fn get_meetings(app: AppHandle) -> Vec<crate::calendar::Meeting> {
    app.state::<AppState>().meetings()
}

#[tauri::command]
pub fn open_join_url(app: AppHandle, url: String) -> Result<(), String> {
    if !crate::calendar::is_valid_teams_url(&url) {
        return Err("Invalid Teams join URL".to_string());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_calendar_settings(app: AppHandle, ics_url: String) -> Result<(), String> {
    if !ics_url.trim().is_empty() {
        let parsed = reqwest::Url::parse(&ics_url).map_err(|e| e.to_string())?;
        if parsed.scheme() != "https" {
            return Err("ICS URL must use HTTPS".to_string());
        }
    }
    let settings = crate::settings::Settings {
        base_url: app.state::<AppState>().base_url(),
        ics_url: ics_url.clone(),
        busy_session_policy: app
            .state::<AppState>()
            .busy_session_policy
            .lock()
            .expect("policy state poisoned")
            .clone(),
        trigger_smart_home: *app
            .state::<AppState>()
            .trigger_smart_home
            .lock()
            .expect("smart home state poisoned"),
        quiet_hours_start: app
            .state::<AppState>()
            .quiet_hours_start
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        quiet_hours_end: app
            .state::<AppState>()
            .quiet_hours_end
            .lock()
            .expect("quiet hours state poisoned")
            .clone(),
        auto_start: app.state::<AppState>().auto_start(),
        discreet_mode: *app
            .state::<AppState>()
            .discreet_mode
            .lock()
            .expect("discreet state poisoned"),
    };
    crate::settings::save(&app, &settings)?;
    app.state::<AppState>().set_ics_url(ics_url);
    tauri::async_runtime::spawn(crate::calendar::refresh(app.clone()));
    Ok(())
}

#[tauri::command]
pub async fn test_connection(base_url: String) -> Result<String, String> {
    let client = BusyBarClient::new(&base_url).map_err(|error| error.to_string())?;
    let status = client.status().await.map_err(|error| error.to_string())?;
    Ok(status.name.unwrap_or_else(|| "Busy Bar".to_string()))
}

#[tauri::command]
pub fn open_bar_ui(app: AppHandle) -> Result<(), String> {
    let url = app.state::<AppState>().base_url();
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|error| error.to_string())
}

fn notify_failure(app: &AppHandle, message: &str) {
    if let Err(notification_error) = notify(app, &format!("Échec : {message}")) {
        eprintln!("Could not show failure notification: {notification_error}");
    }
}

fn notify(app: &AppHandle, message: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title("Meeting Display")
        .body(message)
        .show()
        .map_err(|error| error.to_string())
}
