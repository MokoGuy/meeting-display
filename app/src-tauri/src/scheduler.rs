use crate::busybar::{BusyBarClient, DrawRequest, Error};
use crate::calendar::Meeting;
use crate::meeting_state::{restore_snapshot, MeetingState};
use crate::screens::{self, ScreenKind, APP};
use crate::state::AppState;
use chrono::NaiveTime;
use chrono::{DateTime, Duration, Local, Utc};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

/// Seconds between two scheduler passes.
const TICK_S: u64 = 5;
/// An unchanged screen is re-sent this often so its elements never time out.
const KEEPALIVE_S: u64 = 45;
/// Volume (0-100) our sounds are played at when the bar is set lower than that.
const SOUND_VOLUME: f64 = 50.0;
/// Time left for a built-in sound to finish before the volume is put back.
const SOUND_LENGTH_MS: u64 = 3500;

/// Plays a built-in sound so that it can be heard: the bar is often kept
/// almost silent, so the volume is raised for the sound and then put back.
/// Runs on its own task so the scheduler never waits for the sound.
pub(crate) fn play_audibly(base_url: String, sound: screens::Sound) {
    tauri::async_runtime::spawn(async move {
        let Ok(client) = BusyBarClient::new(&base_url) else {
            return;
        };
        let previous = client.volume().await.ok();
        let raised = match previous {
            Some(volume) if volume < SOUND_VOLUME => client.set_volume(SOUND_VOLUME).await.is_ok(),
            _ => false,
        };
        let _ = match sound {
            screens::Sound::Stock(path) => client.play_stock_sound(APP, path).await,
            screens::Sound::Asset(file) => client.play_asset_sound(APP, file).await,
        };
        if let (true, Some(volume)) = (raised, previous) {
            tokio::time::sleep(std::time::Duration::from_millis(SOUND_LENGTH_MS)).await;
            let _ = client.set_volume(volume).await;
        }
    });
}

/// What is currently on the bar on our behalf.
#[derive(Debug, Clone)]
pub struct Drawn {
    pub kind: ScreenKind,
    pub request: DrawRequest,
    pub at: Instant,
}

pub fn transliterate_subject(subject: &str, max_chars: usize) -> String {
    let converted: String = subject
        .chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'a',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            '’' | '‘' | '“' | '”' | '–' | '—' => '\'',
            c if c.is_ascii() && (c.is_ascii_graphic() || c == ' ') => c,
            _ => ' ',
        })
        .collect();
    converted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

pub fn in_quiet_hours(now_local: NaiveTime, start: &str, end: &str) -> bool {
    let (Ok(start), Ok(end)) = (
        NaiveTime::parse_from_str(start, "%H:%M"),
        NaiveTime::parse_from_str(end, "%H:%M"),
    ) else {
        return false;
    };
    if start == end {
        return false;
    }
    if start < end {
        now_local >= start && now_local < end
    } else {
        now_local >= start || now_local < end
    }
}

pub fn suppress_bar(now_local: NaiveTime, start: &str, end: &str, discreet: bool) -> bool {
    discreet || in_quiet_hours(now_local, start, end)
}

fn notify_armed(app: &tauri::AppHandle, meeting: &Meeting) {
    let subject = transliterate_subject(&meeting.subject, 60);
    let _ = app
        .notification()
        .builder()
        .title("Réunion")
        .body(format!("{subject} — appuyez sur Start"))
        .show();
}

pub async fn run(app: tauri::AppHandle) {
    loop {
        tick(&app).await;
        tokio::time::sleep(std::time::Duration::from_secs(TICK_S)).await;
    }
}

#[derive(Debug, PartialEq)]
pub enum Transition {
    HoldInMeeting,
    RestoreAndOverrun {
        snapshot: Option<serde_json::Value>,
        started_at: DateTime<Utc>,
        meeting: Meeting,
        end: DateTime<Utc>,
    },
    Arm {
        meeting: Meeting,
    },
    DrawOverrun {
        meeting: Meeting,
    },
    ClearOverrunToIdle,
    ArmedEndedToIdle,
    Reminders,
}

/// Pure decision for one scheduler tick: given the current meeting state, the
/// known meetings and the current time, decide what should happen. All device
/// I/O lives in `tick`, which executes the returned transition.
pub fn plan(current: &MeetingState, meetings: &[Meeting], now: DateTime<Utc>) -> Transition {
    if let MeetingState::InMeeting {
        meeting,
        end,
        saved_snapshot,
        started_at,
    } = current
    {
        if now >= *end {
            return Transition::RestoreAndOverrun {
                snapshot: saved_snapshot.clone(),
                started_at: *started_at,
                meeting: meeting.clone(),
                end: *end,
            };
        }
        return Transition::HoldInMeeting;
    }
    if let Some(meeting) = meetings
        .iter()
        .find(|meeting| meeting.start <= now && now < meeting.end)
    {
        return Transition::Arm {
            meeting: meeting.clone(),
        };
    }
    match current {
        MeetingState::Overrun { meeting, end, .. } => {
            if now > *end + Duration::minutes(30) {
                Transition::ClearOverrunToIdle
            } else {
                Transition::DrawOverrun {
                    meeting: meeting.clone(),
                }
            }
        }
        MeetingState::Armed { .. } => Transition::ArmedEndedToIdle,
        _ => Transition::Reminders,
    }
}

/// Number shown in the calendar icon: meetings not yet finished on the same
/// local day as `meeting` (the published ICS carries no attendee list).
pub fn badge_for(meetings: &[Meeting], meeting: &Meeting, now: DateTime<Utc>) -> u32 {
    let day = meeting.start.with_timezone(&Local).date_naive();
    meetings
        .iter()
        .filter(|m| m.end > now && m.start.with_timezone(&Local).date_naive() == day)
        .count() as u32
}

/// Chooses what to show once the state machine has spoken: the active meeting
/// screen if any, otherwise the next upcoming meeting. An overrun gives way as
/// soon as the next meeting gets close.
pub fn pick_view(
    active: Option<(ScreenKind, Meeting)>,
    meetings: &[Meeting],
    now: DateTime<Utc>,
) -> Option<(ScreenKind, Meeting)> {
    let next = meetings
        .iter()
        .filter(|meeting| meeting.start > now)
        .min_by_key(|meeting| meeting.start)
        .map(|meeting| (ScreenKind::before_start(meeting, now), meeting.clone()));
    match (active, next) {
        (Some((ScreenKind::Overrun, _)), Some(next)) if next.0 != ScreenKind::Idle => Some(next),
        (Some(active), _) => Some(active),
        (None, next) => next,
    }
}

/// Puts `view` on the bar (or removes our screen when `None`), sending a
/// request only when something changed or the keep-alive is due.
async fn show(state: &AppState, view: Option<(ScreenKind, DrawRequest)>) {
    let last = state
        .last_screen
        .lock()
        .expect("screen state poisoned")
        .clone();
    let Ok(client) = BusyBarClient::new(&state.base_url()) else {
        return;
    };
    let Some((kind, request)) = view else {
        if last.is_some() {
            let _guard = state.draw_lock.lock().await;
            if state.is_shutting_down() {
                return;
            }
            let _ = client.clear(APP).await;
            *state.last_screen.lock().expect("screen state poisoned") = None;
        }
        return;
    };
    let mut keepalive = false;
    if let Some(previous) = &last {
        if previous.request == request {
            if previous.at.elapsed().as_secs() < KEEPALIVE_S {
                return;
            }
            keepalive = true;
        }
    }
    let _guard = state.draw_lock.lock().await;
    if state.is_shutting_down() {
        return;
    }
    if !state.icon_uploaded.load(Ordering::SeqCst) {
        for (file, bytes) in screens::ASSETS {
            if client.upload_asset(APP, file, bytes).await.is_err() {
                return;
            }
        }
        state.icon_uploaded.store(true, Ordering::SeqCst);
    }
    let entering = last.as_ref().map(|previous| previous.kind) != Some(kind);
    if entering {
        let _ = client.clear(APP).await;
    }
    let mut outgoing = request.clone();
    if keepalive && kind == ScreenKind::Overrun {
        // Do not blink the LED again at every keep-alive during an overrun.
        outgoing.led_notification_color = None;
    }
    match client.draw(&outgoing).await {
        Ok(()) => {
            if entering {
                if let Some(sound) = screens::entry_sound(kind) {
                    play_audibly(state.base_url(), sound);
                }
            }
            *state.last_screen.lock().expect("screen state poisoned") = Some(Drawn {
                kind,
                request,
                at: Instant::now(),
            });
        }
        Err(error) => {
            // 400 usually means the icon is no longer on the bar: upload it again next time.
            if matches!(error, Error::Http { status: 400, .. }) {
                state.icon_uploaded.store(false, Ordering::SeqCst);
            }
            *state.last_screen.lock().expect("screen state poisoned") = None;
        }
    }
}

pub async fn tick(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    if state.is_shutting_down() {
        return;
    }
    let now = Utc::now();
    let quiet_start = state
        .quiet_hours_start
        .lock()
        .expect("quiet hours state poisoned")
        .clone();
    let quiet_end = state
        .quiet_hours_end
        .lock()
        .expect("quiet hours state poisoned")
        .clone();
    let discreet = *state.discreet_mode.lock().expect("discreet state poisoned");
    let suppressed = suppress_bar(
        now.with_timezone(&Local).time(),
        &quiet_start,
        &quiet_end,
        discreet,
    );
    let meetings = state.visible_meetings();
    let current = state
        .meeting_state
        .lock()
        .expect("meeting state poisoned")
        .clone();

    let active = match plan(&current, &meetings, now) {
        Transition::HoldInMeeting => match &current {
            MeetingState::InMeeting { meeting, .. } => {
                Some((ScreenKind::InMeeting, meeting.clone()))
            }
            _ => None,
        },
        Transition::RestoreAndOverrun {
            snapshot,
            started_at,
            meeting,
            end,
        } => {
            *state.meeting_state.lock().expect("meeting state poisoned") = MeetingState::Overrun {
                meeting: meeting.clone(),
                end,
                saved_snapshot: snapshot.clone(),
            };
            if let Some(snap) = snapshot {
                if let Ok(client) = BusyBarClient::new(&state.base_url()) {
                    let _guard = state.draw_lock.lock().await;
                    if state.is_shutting_down() {
                        return;
                    }
                    let _ = client
                        .put_snapshot(&restore_snapshot(&snap, now, started_at))
                        .await;
                }
            }
            // Only a meeting the user confirmed (LIVE) reaches this transition.
            if !suppressed {
                play_audibly(state.base_url(), screens::MEETING_END_SOUND);
            }
            Some((ScreenKind::Overrun, meeting))
        }
        Transition::Arm { meeting } if state.auto_start() => {
            // Automatic start: skip the START screen and go live at once.
            *state.meeting_state.lock().expect("meeting state poisoned") =
                MeetingState::InMeeting {
                    meeting: meeting.clone(),
                    end: meeting.end,
                    saved_snapshot: None,
                    started_at: now,
                };
            if !suppressed {
                play_audibly(state.base_url(), screens::MEETING_START_SOUND);
            }
            Some((ScreenKind::InMeeting, meeting))
        }
        Transition::Arm { meeting } => {
            {
                let mut guard = state.meeting_state.lock().expect("meeting state poisoned");
                if !matches!(&*guard, MeetingState::Armed { meeting: m } if m.id == meeting.id) {
                    *guard = MeetingState::Armed {
                        meeting: meeting.clone(),
                    };
                }
            }
            let key = format!("armed-{}", meeting.id);
            if !state.reminder_was_shown(&key) {
                notify_armed(app, &meeting);
                state.mark_reminder_shown(key);
            }
            Some((ScreenKind::Armed, meeting))
        }
        Transition::DrawOverrun { meeting } => Some((ScreenKind::Overrun, meeting)),
        Transition::ClearOverrunToIdle | Transition::ArmedEndedToIdle => {
            *state.meeting_state.lock().expect("meeting state poisoned") = MeetingState::Idle;
            None
        }
        Transition::Reminders => None,
    };

    let view = if suppressed {
        None
    } else {
        pick_view(active, &meetings, now).map(|(kind, meeting)| {
            let badge = badge_for(&meetings, &meeting, now);
            (kind, screens::build(kind, &meeting, now, badge))
        })
    };
    show(&state, view).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting(start: DateTime<Utc>) -> Meeting {
        Meeting {
            id: "m1".into(),
            subject: "Réunion équipe".into(),
            start,
            end: start + Duration::minutes(30),
            is_online: true,
            join_url: None,
            show_as: "busy".into(),
            response: "accepted".into(),
            is_cancelled: false,
            is_all_day: false,
        }
    }

    #[test]
    fn quiet_hours_support_disabled_and_midnight_windows() {
        assert!(!in_quiet_hours(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            "",
            ""
        ));
        assert!(in_quiet_hours(
            NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            "22:00",
            "08:00"
        ));
        assert!(in_quiet_hours(
            NaiveTime::from_hms_opt(7, 59, 0).unwrap(),
            "22:00",
            "08:00"
        ));
        assert!(!in_quiet_hours(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            "22:00",
            "08:00"
        ));
        assert!(suppress_bar(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            "",
            "",
            true
        ));
    }

    #[test]
    fn transliterates_to_printable_ascii() {
        assert_eq!(
            transliterate_subject("Réunion  d’équipe : été", 60),
            "Reunion d'equipe : ete"
        );
        assert_eq!(transliterate_subject("abcdef", 3), "abc");
    }

    #[test]
    fn view_prefers_active_meeting_then_next_one() {
        let now = DateTime::parse_from_rfc3339("2030-01-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let far = meeting(now + Duration::hours(3));
        let close = meeting(now + Duration::minutes(10));
        let ended = meeting(now - Duration::minutes(40));
        // Nothing active: the next meeting, with the screen its distance calls for.
        let (kind, m) = pick_view(None, &[far.clone(), close.clone()], now).expect("view");
        assert_eq!((kind, m.start), (ScreenKind::Approach, close.start));
        assert!(pick_view(None, &[ended.clone()], now).is_none());
        // An armed meeting stays on screen whatever comes next.
        let armed = Some((ScreenKind::Armed, ended.clone()));
        assert_eq!(
            pick_view(armed, &[close.clone()], now).unwrap().0,
            ScreenKind::Armed
        );
        // An overrun yields to a close meeting, not to a distant one.
        let over = || Some((ScreenKind::Overrun, ended.clone()));
        assert_eq!(
            pick_view(over(), &[close], now).unwrap().0,
            ScreenKind::Approach
        );
        assert_eq!(
            pick_view(over(), &[far], now).unwrap().0,
            ScreenKind::Overrun
        );
    }

    #[test]
    fn badge_counts_unfinished_meetings_of_that_day() {
        let now = DateTime::parse_from_rfc3339("2030-01-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let done = meeting(now - Duration::hours(2));
        let next = meeting(now + Duration::minutes(30));
        let later = meeting(now + Duration::minutes(90));
        let other_day = meeting(now + Duration::days(3));
        let all = [done, next.clone(), later, other_day.clone()];
        assert_eq!(badge_for(&all, &next, now), 2);
        assert_eq!(badge_for(&all, &other_day, now), 1);
    }

    #[test]
    fn plan_covers_meeting_lifecycle() {
        let base = DateTime::parse_from_rfc3339("2030-01-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let m = meeting(base); // start = base, end = base + 30 min
        let mm = vec![m.clone()];
        let idle = MeetingState::Idle;
        let armed = MeetingState::Armed { meeting: m.clone() };
        let inm = MeetingState::InMeeting {
            meeting: m.clone(),
            end: m.end,
            saved_snapshot: None,
            started_at: base,
        };
        let over = MeetingState::Overrun {
            meeting: m.clone(),
            end: m.end,
            saved_snapshot: None,
        };

        // Upcoming meeting -> reminders.
        assert_eq!(
            plan(&idle, &mm, base - Duration::minutes(5)),
            Transition::Reminders
        );
        // Meeting in progress -> Armed.
        assert!(matches!(
            plan(&idle, &mm, base + Duration::minutes(1)),
            Transition::Arm { .. }
        ));
        // Running native timer, not yet ended -> hold.
        assert_eq!(
            plan(&inm, &mm, base + Duration::minutes(5)),
            Transition::HoldInMeeting
        );
        // Native timer ended -> restore + overrun.
        assert!(matches!(
            plan(&inm, &mm, m.end + Duration::minutes(1)),
            Transition::RestoreAndOverrun { .. }
        ));
        // Overrun within grace -> draw; past grace -> idle.
        assert!(matches!(
            plan(&over, &[], m.end + Duration::minutes(5)),
            Transition::DrawOverrun { .. }
        ));
        assert_eq!(
            plan(&over, &[], m.end + Duration::minutes(31)),
            Transition::ClearOverrunToIdle
        );
        // Armed meeting ended without start -> idle.
        assert_eq!(
            plan(&armed, &[], m.end + Duration::minutes(1)),
            Transition::ArmedEndedToIdle
        );
        // Back-to-back: overrun + a new ongoing meeting -> Armed for it.
        let next = meeting(m.end);
        assert!(matches!(
            plan(&over, &[next], m.end + Duration::minutes(2)),
            Transition::Arm { .. }
        ));
    }
}
