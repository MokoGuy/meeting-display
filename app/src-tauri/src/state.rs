use crate::busybar::Status;
use crate::calendar::Meeting;
use crate::meeting_state::MeetingState;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub connection: Mutex<ConnectionState>,
    pub base_url: Mutex<String>,
    pub ics_url: Mutex<String>,
    pub meetings: Mutex<Vec<Meeting>>,
    pub shown_reminders: Mutex<HashSet<String>>,
    /// Set on app exit so the scheduler stops drawing before/after cleanup.
    pub shutting_down: AtomicBool,
    /// Serializes the scheduler's clear+draw with the exit cleanup so no draw
    /// can land after the final clear.
    pub draw_lock: tokio::sync::Mutex<()>,
    pub meeting_state: Mutex<MeetingState>,
    pub busy_session_policy: Mutex<String>,
    pub trigger_smart_home: Mutex<bool>,
    pub quiet_hours_start: Mutex<String>,
    pub quiet_hours_end: Mutex<String>,
    pub discreet_mode: Mutex<bool>,
    pub auto_start: Mutex<bool>,
    /// Meetings the user skipped or ended by hand: ignored until the app restarts.
    /// What the user did with their meetings (held or skipped), saved on disk.
    pub history: Mutex<crate::history::History>,
    /// What the scheduler last drew on the bar, and when (None = nothing of ours on screen).
    pub last_screen: Mutex<Option<crate::scheduler::Drawn>>,
    /// Whether our assets (icon and sounds) have been stored on the bar since the last failure.
    pub icon_uploaded: AtomicBool,
}

#[derive(Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub last_status: Option<Status>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionDto {
    pub connected: bool,
    pub name: Option<String>,
    pub battery: Option<u8>,
    pub power_state: Option<String>,
}

impl AppState {
    pub fn new(
        base_url: String,
        ics_url: String,
        busy_session_policy: String,
        trigger_smart_home: bool,
        quiet_hours_start: String,
        quiet_hours_end: String,
        discreet_mode: bool,
        auto_start: bool,
    ) -> Self {
        Self {
            connection: Mutex::new(ConnectionState::default()),
            base_url: Mutex::new(base_url),
            ics_url: Mutex::new(ics_url),
            meetings: Mutex::new(Vec::new()),
            shown_reminders: Mutex::new(HashSet::new()),
            shutting_down: AtomicBool::new(false),
            draw_lock: tokio::sync::Mutex::new(()),
            meeting_state: Mutex::new(MeetingState::Idle),
            busy_session_policy: Mutex::new(busy_session_policy),
            trigger_smart_home: Mutex::new(trigger_smart_home),
            quiet_hours_start: Mutex::new(quiet_hours_start),
            quiet_hours_end: Mutex::new(quiet_hours_end),
            discreet_mode: Mutex::new(discreet_mode),
            auto_start: Mutex::new(auto_start),
            history: Mutex::new(crate::history::History::default()),
            last_screen: Mutex::new(None),
            icon_uploaded: AtomicBool::new(false),
        }
    }

    pub fn auto_start(&self) -> bool {
        *self.auto_start.lock().expect("auto start state poisoned")
    }

    pub fn set_shutting_down(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    /// Undo `set_shutting_down` when the app stays alive after all.
    pub fn resume(&self) {
        self.shutting_down.store(false, Ordering::SeqCst);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    pub fn reminder_was_shown(&self, key: &str) -> bool {
        self.shown_reminders
            .lock()
            .expect("reminder state poisoned")
            .contains(key)
    }

    pub fn mark_reminder_shown(&self, key: String) {
        self.shown_reminders
            .lock()
            .expect("reminder state poisoned")
            .insert(key);
    }

    pub fn ics_url(&self) -> String {
        self.ics_url.lock().expect("ICS URL state poisoned").clone()
    }

    pub fn set_ics_url(&self, url: String) {
        *self.ics_url.lock().expect("ICS URL state poisoned") = url;
    }

    pub fn meetings(&self) -> Vec<Meeting> {
        self.meetings
            .lock()
            .expect("meetings state poisoned")
            .clone()
    }

    /// Reads the saved history; called once at startup.
    pub fn load_history(&self, path: std::path::PathBuf) {
        *self.history.lock().expect("history state poisoned") =
            crate::history::History::load(path, chrono::Utc::now());
    }

    /// Known meetings minus the ones already held or skipped.
    pub fn visible_meetings(&self) -> Vec<Meeting> {
        let history = self.history.lock().expect("history state poisoned");
        self.meetings()
            .into_iter()
            .filter(|meeting| !history.contains(&meeting.id))
            .collect()
    }

    /// Meeting id to "done" or "skipped", for the meetings set aside by the user.
    pub fn dismissed_map(&self) -> HashMap<String, String> {
        self.history
            .lock()
            .expect("history state poisoned")
            .states()
    }

    /// Every recorded meeting of the last 30 days, oldest first.
    pub fn history_entries(&self) -> Vec<crate::history::Entry> {
        let mut history = self.history.lock().expect("history state poisoned");
        history.prune(chrono::Utc::now());
        history.entries()
    }

    /// Puts one meeting back in play.
    pub fn resume_meeting(&self, meeting_id: &str) {
        self.history
            .lock()
            .expect("history state poisoned")
            .remove(meeting_id);
    }

    pub fn record_skipped(&self, meeting: &Meeting) {
        self.history
            .lock()
            .expect("history state poisoned")
            .record(crate::history::Entry::skipped(meeting), chrono::Utc::now());
    }

    /// A meeting that was held, from when it was started to when it ended.
    pub fn record_done(
        &self,
        meeting: &Meeting,
        started_at: chrono::DateTime<chrono::Utc>,
        ended_at: chrono::DateTime<chrono::Utc>,
    ) {
        self.history.lock().expect("history state poisoned").record(
            crate::history::Entry::done(meeting, started_at, ended_at),
            chrono::Utc::now(),
        );
    }

    /// The user ended an overrunning meeting: its real end is now.
    pub fn close_done(&self, meeting_id: &str, ended_at: chrono::DateTime<chrono::Utc>) {
        self.history
            .lock()
            .expect("history state poisoned")
            .close(meeting_id, ended_at);
    }

    pub fn set_meetings(&self, meetings: Vec<Meeting>) {
        *self.meetings.lock().expect("meetings state poisoned") = meetings;
    }

    pub fn base_url(&self) -> String {
        self.base_url
            .lock()
            .expect("base URL state poisoned")
            .clone()
    }

    pub fn set_base_url(&self, base_url: String) {
        *self.base_url.lock().expect("base URL state poisoned") = base_url;
    }

    pub fn set_connected(&self, status: Status) {
        let mut state = self.connection.lock().expect("connection state poisoned");
        state.connected = true;
        state.last_status = Some(status);
    }

    pub fn set_offline(&self) {
        let mut state = self.connection.lock().expect("connection state poisoned");
        state.connected = false;
        state.last_status = None;
    }

    pub fn dto(&self) -> ConnectionDto {
        let state = self.connection.lock().expect("connection state poisoned");
        ConnectionDto {
            connected: state.connected,
            name: state
                .last_status
                .as_ref()
                .and_then(|status| status.name.clone()),
            battery: state
                .last_status
                .as_ref()
                .map(|status| status.battery_charge),
            power_state: state
                .last_status
                .as_ref()
                .map(|status| status.power_state.clone()),
        }
    }
}
