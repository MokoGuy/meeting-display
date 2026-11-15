//! Link with the on-bar companion app (`bar-app/`): the JS app that draws the
//! screens on the bar itself and reads the wheel and buttons.
//!
//! Everything goes through three small files in the companion's data folder,
//! using the bar's storage API:
//! - `agenda.json`, written here: what the companion must show;
//! - `actions.txt`, appended by the companion: what the user did on the bar;
//! - `alive.txt`, rewritten by the companion: proof that it owns the screen.
//!
//! While the companion is alive this app stops drawing and only keeps the
//! agenda up to date; when it goes away, this app draws again.

use crate::busybar::BusyBarClient;
use crate::calendar::Meeting;
use crate::history::Entry;
use crate::meeting_state::{started_now, MeetingState};
use crate::scheduler::{badge_for, transliterate_subject};
use crate::state::AppState;
use chrono::{DateTime, Datelike, Local, Timelike, Utc};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::Manager;

/// Id of the companion app on the bar (see `bar-app/src/appmeta/manifest.json`).
pub const APP_ID: &str = "meeting-display";
const AGENDA: &str = "/ext/apps_data/js_apps/meeting-display/agenda.json";
const ACTIONS: &str = "/ext/apps_data/js_apps/meeting-display/actions.txt";
const ALIVE: &str = "/ext/apps_data/js_apps/meeting-display/alive.txt";

/// The companion rewrites its heartbeat every 20 s: silent for longer than
/// this, it is considered gone.
const ALIVE_FOR: Duration = Duration::from_secs(60);
/// The actions file is cleared once it has grown past this many lines.
const ACTIONS_MAX_LINES: usize = 40;
/// Meetings listed for the wheel.
const DAY_MAX_ITEMS: usize = 16;

/// What this app remembers about the companion between two passes.
#[derive(Debug, Default)]
pub struct Link {
    last_alive: Option<String>,
    alive_seen_at: Option<Instant>,
    /// Highest action sequence number already applied; `None` until the
    /// actions file has been read once, so old lines are never replayed.
    last_seq: Option<u64>,
    last_agenda: String,
    /// Screen the companion is showing, to play the entry sounds here.
    pub last_kind: Option<crate::screens::ScreenKind>,
}

impl Link {
    /// Records the heartbeat just read and tells whether the companion is alive.
    fn note_alive(&mut self, alive: Option<String>, now: Instant) -> bool {
        match alive {
            None => {
                self.last_alive = None;
                self.alive_seen_at = None;
            }
            Some(value) => {
                if self.last_alive.as_deref() != Some(value.as_str()) {
                    // First sight of a heartbeat only counts once it moves:
                    // a stale file left by a closed companion never does.
                    if self.last_alive.is_some() {
                        self.alive_seen_at = Some(now);
                    }
                    self.last_alive = Some(value);
                }
            }
        }
        self.is_alive(now)
    }

    pub fn is_alive(&self, now: Instant) -> bool {
        self.alive_seen_at
            .is_some_and(|seen| now.duration_since(seen) < ALIVE_FOR)
    }
}

fn hhmm(time: DateTime<Local>) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn weekday_fr(date: DateTime<Local>) -> &'static str {
    ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"]
        [date.weekday().num_days_from_monday() as usize]
}

/// The agenda the companion shows: the meeting that matters now, and today's
/// meetings for the wheel. Pure, so it can be tested.
pub fn build_agenda(
    current: &MeetingState,
    visible: &[Meeting],
    history: &[Entry],
    now: DateTime<Utc>,
) -> Value {
    let today = now.with_timezone(&Local).date_naive();
    let (phase, meeting) = match current {
        MeetingState::Armed { meeting } => ("armed", Some(meeting.clone())),
        MeetingState::InMeeting { meeting, .. } => ("live", Some(meeting.clone())),
        MeetingState::Overrun { meeting, .. } => ("overrun", Some(meeting.clone())),
        MeetingState::Idle => (
            "upcoming",
            visible
                .iter()
                .filter(|meeting| meeting.start > now)
                .min_by_key(|meeting| meeting.start)
                .cloned(),
        ),
    };
    let cur = meeting.as_ref().map(|meeting| {
        let start = meeting.start.with_timezone(&Local);
        let is_today = start.date_naive() == today;
        json!({
            "id": meeting.id,
            "t": transliterate_subject(&meeting.subject, 48),
            "hm": hhmm(start),
            "lbl": if is_today { hhmm(start) } else { format!("{} {}", weekday_fr(start), hhmm(start)) },
            "s": meeting.start.timestamp(),
            "e": meeting.end.timestamp(),
            "phase": phase,
            "badge": badge_for(visible, meeting, now),
            "tent": meeting.show_as == "tentative",
            "today": is_today,
        })
    });

    // Today, in time order: what was done, then what is still to come.
    let mut day: Vec<(DateTime<Utc>, Value)> = Vec::new();
    for entry in history {
        if entry.start.with_timezone(&Local).date_naive() != today {
            continue;
        }
        // The meeting on screen is listed once, as itself.
        if meeting
            .as_ref()
            .is_some_and(|current| current.id == entry.id)
        {
            continue;
        }
        day.push((
            entry.start,
            json!({
                "id": entry.id,
                "t": transliterate_subject(&entry.subject, 48),
                "hm": hhmm(entry.start.with_timezone(&Local)),
                "st": if entry.state == "done" { "done" } else { "skip" },
            }),
        ));
    }
    let mut listed: Vec<Meeting> = visible
        .iter()
        .filter(|meeting| meeting.end > now)
        .filter(|meeting| meeting.start.with_timezone(&Local).date_naive() == today)
        .cloned()
        .collect();
    if let Some(current) = &meeting {
        if current.start.with_timezone(&Local).date_naive() == today
            && !listed.iter().any(|meeting| meeting.id == current.id)
        {
            listed.push(current.clone());
        }
    }
    for meeting in listed {
        day.push((
            meeting.start,
            json!({
                "id": meeting.id,
                "t": transliterate_subject(&meeting.subject, 48),
                "hm": hhmm(meeting.start.with_timezone(&Local)),
                "st": if meeting.show_as == "tentative" { "tent" } else { "up" },
            }),
        ));
    }
    day.sort_by_key(|(start, _)| *start);
    let day: Vec<Value> = day
        .into_iter()
        .map(|(_, item)| item)
        .take(DAY_MAX_ITEMS)
        .collect();

    let mut agenda = json!({ "cur": cur, "day": day });
    // The companion redraws when this number changes: a digest of the content.
    let digest = agenda.to_string().bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });
    agenda["seq"] = json!(digest);
    agenda
}

/// One line of the actions file: `<seq>|<action>|<meeting id>`.
pub fn parse_action(line: &str) -> Option<(u64, String, String)> {
    let mut parts = line.trim().splitn(3, '|');
    let seq = parts.next()?.parse().ok()?;
    let action = parts.next()?.to_string();
    let id = parts.next()?.to_string();
    (!action.is_empty() && !id.is_empty()).then_some((seq, action, id))
}

/// Applies an action the user did on the bar. Returns whether anything changed.
fn apply(state: &AppState, action: &str, id: &str, now: DateTime<Utc>) -> bool {
    let mut current = state.meeting_state.lock().expect("meeting state poisoned");
    match action {
        "start" => {
            let (meeting, was_armed) = match &*current {
                MeetingState::Armed { meeting } if meeting.id == id => (meeting.clone(), true),
                MeetingState::Idle | MeetingState::Overrun { .. } | MeetingState::Armed { .. } => {
                    let found = state
                        .visible_meetings()
                        .into_iter()
                        .find(|meeting| meeting.id == id && meeting.end > now);
                    match found {
                        Some(meeting) => (meeting, false),
                        None => return false,
                    }
                }
                MeetingState::InMeeting { .. } => return false,
            };
            let meeting = started_now(meeting, now);
            *current = MeetingState::InMeeting {
                end: meeting.end,
                meeting,
                saved_snapshot: None,
                started_at: now,
            };
            if !was_armed {
                crate::scheduler::play_audibly(
                    state.base_url(),
                    crate::screens::MEETING_START_SOUND,
                );
            }
            true
        }
        "skip" => {
            if let MeetingState::Armed { meeting } = &*current {
                if meeting.id == id {
                    state.record_skipped(meeting);
                    *current = MeetingState::Idle;
                    return true;
                }
            }
            let found = state
                .visible_meetings()
                .into_iter()
                .find(|meeting| meeting.id == id && meeting.start > now);
            match found {
                Some(meeting) => {
                    state.record_skipped(&meeting);
                    true
                }
                None => false,
            }
        }
        "end" => match &*current {
            MeetingState::InMeeting {
                meeting,
                started_at,
                ..
            } if meeting.id == id => {
                state.record_done(meeting, *started_at, now);
                *current = MeetingState::Idle;
                true
            }
            MeetingState::Overrun { meeting, .. } if meeting.id == id => {
                state.close_done(&meeting.id, now);
                *current = MeetingState::Idle;
                true
            }
            _ => false,
        },
        "resume" => {
            state.resume_meeting(id);
            true
        }
        _ => false,
    }
}

/// One pass of the link: heartbeat, actions from the bar, agenda to the bar.
/// Returns whether the companion currently owns the screen.
pub async fn sync(app: &tauri::AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Ok(client) = BusyBarClient::new(&state.base_url()) else {
        return false;
    };
    // A missing heartbeat file means the companion is not installed or never
    // ran: nothing else to do, and no error to report.
    let alive = client.storage_read(ALIVE).await.ok().flatten();
    let installed = alive.is_some();
    let active = state
        .companion
        .lock()
        .expect("companion state poisoned")
        .note_alive(alive, Instant::now());
    if !installed {
        return false;
    }

    // Actions done on the bar since the last pass.
    if let Ok(Some(text)) = client.storage_read(ACTIONS).await {
        let now = Utc::now();
        let actions: Vec<_> = text.lines().filter_map(parse_action).collect();
        let last_seq = state
            .companion
            .lock()
            .expect("companion state poisoned")
            .last_seq;
        let newest = actions.iter().map(|(seq, _, _)| *seq).max();
        if let Some(last) = last_seq {
            for (seq, action, id) in &actions {
                if *seq > last {
                    let changed = apply(&state, action, id, now);
                    tracing::info!(companion = "action", action = %action, changed);
                }
            }
        }
        // First read: remember where the file stands, replay nothing.
        state
            .companion
            .lock()
            .expect("companion state poisoned")
            .last_seq = Some(newest.unwrap_or(0).max(last_seq.unwrap_or(0)));
        if text.lines().count() > ACTIONS_MAX_LINES {
            let _ = client.storage_remove(ACTIONS).await;
        }
    } else {
        let mut link = state.companion.lock().expect("companion state poisoned");
        if link.last_seq.is_none() {
            link.last_seq = Some(0);
        }
    }

    // Agenda, rewritten only when it changed.
    let current = state
        .meeting_state
        .lock()
        .expect("meeting state poisoned")
        .clone();
    let agenda = build_agenda(
        &current,
        &state.visible_meetings(),
        &state.history_entries(),
        Utc::now(),
    )
    .to_string();
    let changed = state
        .companion
        .lock()
        .expect("companion state poisoned")
        .last_agenda
        != agenda;
    if changed
        && client
            .storage_write(AGENDA, agenda.as_bytes())
            .await
            .is_ok()
    {
        state
            .companion
            .lock()
            .expect("companion state poisoned")
            .last_agenda = agenda;
    }
    active
}

/// Starts the companion on the bar if it is installed. Called once at startup.
pub async fn launch(app: tauri::AppHandle) {
    tokio::time::sleep(Duration::from_secs(3)).await;
    let state = app.state::<AppState>();
    if let Ok(client) = BusyBarClient::new(&state.base_url()) {
        let launched = client.launch_app(APP_ID).await.is_ok();
        tracing::info!(companion = "launch", launched);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn meeting(id: &str, start: DateTime<Utc>, minutes: i64) -> Meeting {
        Meeting {
            id: id.into(),
            subject: format!("Réunion {id}"),
            start,
            end: start + ChronoDuration::minutes(minutes),
            is_online: false,
            join_url: None,
            show_as: "busy".into(),
            response: "none".into(),
            is_cancelled: false,
            is_all_day: false,
        }
    }

    /// Noon local time today, so every test meeting falls on the same local day.
    fn noon() -> DateTime<Utc> {
        Local::now()
            .with_hour(12)
            .and_then(|t| t.with_minute(0))
            .and_then(|t| t.with_second(0))
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn agenda_lists_today_in_time_order_with_states() {
        let now = noon();
        let done = meeting("done", now - ChronoDuration::hours(2), 30);
        let next = meeting("next", now + ChronoDuration::minutes(20), 30);
        let mut later = meeting("later", now + ChronoDuration::hours(2), 30);
        later.show_as = "tentative".into();
        let tomorrow = meeting("tomorrow", now + ChronoDuration::days(1), 30);
        let history = vec![Entry::done(&done, done.start, done.end)];
        let agenda = build_agenda(
            &MeetingState::Idle,
            &[next.clone(), later, tomorrow],
            &history,
            now,
        );
        assert_eq!(agenda["cur"]["id"], "next");
        assert_eq!(agenda["cur"]["phase"], "upcoming");
        assert_eq!(agenda["cur"]["t"], "Reunion next");
        assert_eq!(agenda["cur"]["today"], true);
        let day: Vec<(String, String)> = agenda["day"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["id"].as_str().unwrap().to_string(),
                    item["st"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(
            day,
            vec![
                ("done".into(), "done".into()),
                ("next".into(), "up".into()),
                ("later".into(), "tent".into())
            ]
        );
    }

    #[test]
    fn agenda_follows_the_meeting_state() {
        let now = noon();
        let m = meeting("m", now - ChronoDuration::minutes(5), 30);
        let live = MeetingState::InMeeting {
            meeting: m.clone(),
            end: m.end,
            saved_snapshot: None,
            started_at: now - ChronoDuration::minutes(4),
        };
        let agenda = build_agenda(&live, &[m.clone()], &[], now);
        assert_eq!(agenda["cur"]["phase"], "live");
        assert_eq!(agenda["day"].as_array().unwrap().len(), 1);
        let empty = build_agenda(&MeetingState::Idle, &[], &[], now);
        assert!(empty["cur"].is_null());
        // The digest changes with the content, and only then.
        assert_ne!(agenda["seq"], empty["seq"]);
        assert_eq!(
            build_agenda(&MeetingState::Idle, &[], &[], now)["seq"],
            empty["seq"]
        );
    }

    #[test]
    fn parses_action_lines() {
        assert_eq!(
            parse_action("1700000000123|start|uid-1-1700000000\n"),
            Some((1700000000123, "start".into(), "uid-1-1700000000".into()))
        );
        // Meeting ids may contain the separator: only the first two are split on.
        assert_eq!(
            parse_action("5|skip|a|b"),
            Some((5, "skip".into(), "a|b".into()))
        );
        assert_eq!(parse_action("garbage"), None);
        assert_eq!(parse_action("x|start|id"), None);
    }

    #[test]
    fn a_stale_heartbeat_file_does_not_count_as_alive() {
        let mut link = Link::default();
        let t0 = Instant::now();
        // The file exists but never changes: the companion is not running.
        assert!(!link.note_alive(Some("100".into()), t0));
        assert!(!link.note_alive(Some("100".into()), t0 + Duration::from_secs(5)));
        // It moves: the companion is alive.
        assert!(link.note_alive(Some("200".into()), t0 + Duration::from_secs(10)));
        assert!(link.is_alive(t0 + Duration::from_secs(40)));
        // No change for more than a minute: gone.
        assert!(!link.note_alive(Some("200".into()), t0 + Duration::from_secs(80)));
        // File removed: gone at once.
        assert!(link.note_alive(Some("300".into()), t0 + Duration::from_secs(90)));
        assert!(!link.note_alive(None, t0 + Duration::from_secs(95)));
    }
}
