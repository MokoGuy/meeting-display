//! Local history of what the user did with their meetings: held ("done") or
//! skipped. Kept in a JSON file in the app data folder, over a rolling 30 days.
//! Nothing here leaves the machine.

use crate::calendar::Meeting;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// How long an entry is kept.
pub const KEEP_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Meeting id (calendar UID plus scheduled start): one entry per occurrence.
    pub id: String,
    pub subject: String,
    /// When the meeting is shown in the agenda: its scheduled start, or the
    /// moment it was started when it was started ahead of time.
    pub start: DateTime<Utc>,
    /// Planned length, in seconds.
    pub planned_secs: i64,
    /// "done" or "skipped".
    pub state: String,
    /// For a meeting that was held: when it really started and ended.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
}

impl Entry {
    pub fn skipped(meeting: &Meeting) -> Self {
        Self {
            id: meeting.id.clone(),
            subject: meeting.subject.clone(),
            start: meeting.start,
            planned_secs: (meeting.end - meeting.start).num_seconds(),
            state: "skipped".into(),
            started_at: None,
            ended_at: None,
        }
    }

    pub fn done(meeting: &Meeting, started_at: DateTime<Utc>, ended_at: DateTime<Utc>) -> Self {
        Self {
            id: meeting.id.clone(),
            subject: meeting.subject.clone(),
            start: meeting.start,
            planned_secs: (meeting.end - meeting.start).num_seconds(),
            state: "done".into(),
            started_at: Some(started_at),
            ended_at: Some(ended_at),
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    entries: Vec<Entry>,
    /// Where the history is saved; `None` keeps it in memory only (tests).
    path: Option<PathBuf>,
}

impl History {
    /// Reads the file if it exists; a missing or unreadable file gives an empty history.
    pub fn load(path: PathBuf, now: DateTime<Utc>) -> Self {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Entry>>(&text).ok())
            .unwrap_or_default();
        let mut history = Self {
            entries,
            path: Some(path),
        };
        history.prune(now);
        history
    }

    /// Drops entries older than the rolling window.
    pub fn prune(&mut self, now: DateTime<Utc>) {
        let oldest = now - Duration::days(KEEP_DAYS);
        self.entries.retain(|entry| entry.start >= oldest);
    }

    /// Adds an entry, replacing any previous one for the same meeting.
    pub fn record(&mut self, entry: Entry, now: DateTime<Utc>) {
        self.entries.retain(|existing| existing.id != entry.id);
        self.entries.push(entry);
        self.prune(now);
        self.save();
    }

    /// Sets the real end of a meeting already recorded as held.
    pub fn close(&mut self, id: &str, ended_at: DateTime<Utc>) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.ended_at = Some(ended_at);
            self.save();
        }
    }

    /// Forgets what was recorded for a meeting, so it is offered again.
    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|entry| entry.id != id);
        self.save();
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|entry| entry.id == id)
    }

    /// Meeting id to "done" or "skipped".
    pub fn states(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.state.clone()))
            .collect()
    }

    /// Every entry, oldest first.
    pub fn entries(&self) -> Vec<Entry> {
        let mut entries = self.entries.clone();
        entries.sort_by_key(|entry| entry.start);
        entries
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&self.entries) {
            Ok(text) => {
                if let Err(error) = std::fs::write(path, text) {
                    tracing::info!(history = "save failed", %error);
                }
            }
            Err(error) => tracing::info!(history = "encode failed", %error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting(id: &str, start: DateTime<Utc>, minutes: i64) -> Meeting {
        Meeting {
            id: id.into(),
            subject: format!("Meeting {id}"),
            start,
            end: start + Duration::minutes(minutes),
            is_online: false,
            join_url: None,
            show_as: "busy".into(),
            response: "none".into(),
            is_cancelled: false,
            is_all_day: false,
        }
    }

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn records_planned_and_actual_length() {
        let now = at("2030-01-10T10:45:00Z");
        let m = meeting("a", at("2030-01-10T10:00:00Z"), 30);
        let mut history = History::default();
        history.record(
            Entry::done(&m, at("2030-01-10T10:02:00Z"), at("2030-01-10T10:44:00Z")),
            now,
        );
        let entry = &history.entries()[0];
        assert_eq!(entry.planned_secs, 30 * 60);
        assert_eq!(
            (entry.ended_at.unwrap() - entry.started_at.unwrap()).num_minutes(),
            42
        );
        assert_eq!(history.states()["a"], "done");
    }

    #[test]
    fn one_entry_per_meeting_and_resume_forgets_it() {
        let now = at("2030-01-10T12:00:00Z");
        let m = meeting("a", at("2030-01-10T11:00:00Z"), 30);
        let mut history = History::default();
        history.record(Entry::skipped(&m), now);
        history.record(Entry::done(&m, m.start, m.end), now);
        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.states()["a"], "done");
        history.remove("a");
        assert!(!history.contains("a"));
    }

    #[test]
    fn close_sets_the_real_end() {
        let now = at("2030-01-10T12:00:00Z");
        let m = meeting("a", at("2030-01-10T11:00:00Z"), 30);
        let mut history = History::default();
        history.record(Entry::done(&m, m.start, m.end), now);
        history.close("a", at("2030-01-10T11:50:00Z"));
        assert_eq!(
            history.entries()[0].ended_at,
            Some(at("2030-01-10T11:50:00Z"))
        );
    }

    #[test]
    fn keeps_a_rolling_thirty_days() {
        let now = at("2030-03-01T12:00:00Z");
        let mut history = History::default();
        history.record(
            Entry::skipped(&meeting("old", now - Duration::days(31), 30)),
            now,
        );
        history.record(
            Entry::skipped(&meeting("edge", now - Duration::days(29), 30)),
            now,
        );
        history.record(
            Entry::skipped(&meeting("recent", now - Duration::hours(2), 30)),
            now,
        );
        let ids: Vec<_> = history.entries().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec!["edge", "recent"]);
    }

    #[test]
    fn survives_a_restart() {
        let dir =
            std::env::temp_dir().join(format!("meeting-display-history-{}", std::process::id()));
        let path = dir.join("history.json");
        let _ = std::fs::remove_file(&path);
        let now = at("2030-01-10T12:00:00Z");
        let m = meeting("a", at("2030-01-10T11:00:00Z"), 30);
        let mut history = History::load(path.clone(), now);
        history.record(Entry::done(&m, m.start, m.end), now);
        let reloaded = History::load(path.clone(), now);
        assert_eq!(reloaded.entries(), history.entries());
        // A month and a day later, the entry has aged out.
        assert!(History::load(path.clone(), now + Duration::days(31))
            .entries()
            .is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
