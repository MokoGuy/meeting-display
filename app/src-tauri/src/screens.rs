//! The six screens of the meeting cinematic, as pure builders.
//!
//! Every screen follows the same template on the 72×16 front display: a 16 px
//! icon lane on the left (calendar icon + a small number), the meeting subject
//! on the top line and one piece of information on the bottom line. The back
//! display (160×80) carries a state label, the full subject and a detail line.
//!
//! Layout and colours were tuned on a real bar (firmware 1.4.1) with a demo
//! script and checked on screen captures. The icon is `calendar.png` from the
//! official Clock app (github.com/busy-app/clock-app, MIT).

use crate::busybar::{DrawElement, DrawRequest};
use crate::calendar::Meeting;
use crate::scheduler::transliterate_subject;
use chrono::{DateTime, Datelike, Local, Timelike, Utc};

pub const APP: &str = "meeting_display";
pub const ICON_FILE: &str = "calendar.png";
pub const ICON_BYTES: &[u8] = include_bytes!("../assets/calendar.png");

/// Seconds after which the bar removes our elements by itself. The scheduler
/// redraws well before that, so the screen only disappears if the app dies.
pub const ELEMENT_TIMEOUT_S: u32 = 90;

const WHITE: &str = "#FFFFFFFF";
const DIM: &str = "#FFFFFF90";
const AMBER: &str = "#FFB000FF";
const RED: &str = "#FF5050FF";
const ORANGE: &str = "#FFA500FF";
const GREEN: &str = "#40E070FF";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    /// Next meeting is more than 15 minutes away.
    Idle,
    /// 15 to 5 minutes before the start.
    Approach,
    /// Last 5 minutes: live countdown; LED during the final minute.
    Imminent,
    /// The meeting has started and waits for the user to press Start.
    Armed,
    /// The user confirmed: time left until the scheduled end.
    InMeeting,
    /// Past the scheduled end.
    Overrun,
}

impl ScreenKind {
    /// Screen for a meeting that has not started yet.
    pub fn before_start(meeting: &Meeting, now: DateTime<Utc>) -> Self {
        let seconds = (meeting.start - now).num_seconds();
        if seconds > 15 * 60 {
            Self::Idle
        } else if seconds > 5 * 60 {
            Self::Approach
        } else {
            Self::Imminent
        }
    }

    /// Every screen must sit above a running JS app such as Clock, which
    /// holds priority 50: measured on firmware 1.4.1, a draw at 50 or below
    /// is refused (409) and 51 is accepted. All stay below the bar's own
    /// BUSY session (90); the screens that need attention rank highest.
    fn priority(self) -> u8 {
        match self {
            Self::Idle => 55,
            Self::Approach => 56,
            Self::Imminent | Self::Armed | Self::InMeeting => 65,
            Self::Overrun => 60,
        }
    }

    fn background(self) -> &'static [&'static str] {
        match self {
            Self::Idle => &["#000000FF"],
            Self::Approach => &["#1E1200FF", "#000000FF"],
            Self::Imminent => &["#4A0808FF", "#140202FF"],
            Self::Armed => &["#5A0000FF", "#1E0000FF"],
            Self::InMeeting => &["#002A12FF", "#000A04FF"],
            Self::Overrun => &["#2A1600FF", "#000000FF"],
        }
    }
}

/// A sound the bar can play: one of its built-in files, or one of ours
/// uploaded next to the icon (see `ASSETS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Stock(&'static str),
    Asset(&'static str),
}

/// Files stored in our assets on the bar before the first draw. The .snd
/// files are raw 16-bit signed little-endian mono PCM at 44.1 kHz.
pub const ASSETS: &[(&str, &[u8])] = &[
    (ICON_FILE, ICON_BYTES),
    ("doux.snd", include_bytes!("../assets/doux.snd")),
    ("carillon.snd", include_bytes!("../assets/carillon.snd")),
];

/// The bar's own "event starts" chime, played when the meeting begins.
pub const MEETING_START_SOUND: Sound = Sound::Stock("shared/calendar_event_starts.snd");

/// Played when a meeting the user had confirmed (LIVE) reaches its scheduled end.
pub const MEETING_END_SOUND: Sound = Sound::Asset("carillon.snd");

/// Sound played once when a screen appears: a soft note when the meeting
/// gets close (15 min), the bar's reminder chime for the last five minutes
/// and its "event starts" chime when the meeting begins.
pub fn entry_sound(kind: ScreenKind) -> Option<Sound> {
    match kind {
        ScreenKind::Approach => Some(Sound::Asset("doux.snd")),
        ScreenKind::Imminent => Some(Sound::Stock("shared/calendar_reminder_ends.snd")),
        ScreenKind::Armed => Some(MEETING_START_SOUND),
        _ => None,
    }
}

/// "2h05", "12 min", "45 min" — time left before a start, rounded up to the minute.
pub fn remaining_label(seconds: i64) -> String {
    let minutes = (seconds.max(0) + 59) / 60;
    if minutes >= 60 {
        format!("{}h{:02}", minutes / 60, minutes % 60)
    } else {
        format!("{minutes} min")
    }
}

fn weekday_fr(date: DateTime<Local>) -> &'static str {
    ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"]
        [date.weekday().num_days_from_monday() as usize]
}

fn hhmm(time: DateTime<Local>) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn text(
    id: &str,
    value: String,
    font: &str,
    x: i32,
    y: i32,
    align: &str,
    color: &str,
) -> DrawElement {
    DrawElement::Text {
        id: id.into(),
        timeout: ELEMENT_TIMEOUT_S,
        x,
        y,
        align: align.into(),
        display: "front".into(),
        text: value,
        font: font.into(),
        color: color.into(),
        width: None,
        scroll_rate: None,
        scroll_start_delay: None,
        scroll_repeat_delay: None,
    }
}

fn scrolling(
    id: &str,
    value: String,
    font: &str,
    display: &str,
    x: i32,
    y: i32,
    width: u32,
    color: &str,
) -> DrawElement {
    DrawElement::Text {
        id: id.into(),
        timeout: ELEMENT_TIMEOUT_S,
        x,
        y,
        align: "top_left".into(),
        display: display.into(),
        text: value,
        font: font.into(),
        color: color.into(),
        width: Some(width),
        scroll_rate: Some(1200),
        scroll_start_delay: Some(1500),
        scroll_repeat_delay: Some(2500),
    }
}

fn countdown(timestamp: i64, direction: &str, color: &str, x: i32) -> DrawElement {
    DrawElement::Countdown {
        id: "cd".into(),
        timeout: ELEMENT_TIMEOUT_S,
        x,
        y: 19,
        align: "bottom_left".into(),
        display: "front".into(),
        timestamp: timestamp.to_string(),
        color: color.into(),
        direction: direction.into(),
        show_hours: "when_non_zero".into(),
    }
}

fn back_line(id: &str, value: String, font: &str, y: i32) -> DrawElement {
    scrolling(id, value, font, "back", 4, y, 136, WHITE)
}

/// Builds the draw request for one screen.
///
/// `badge` is the small number shown inside the calendar icon (the scheduler
/// passes the day of the month of the meeting).
pub fn build(kind: ScreenKind, meeting: &Meeting, now: DateTime<Utc>, badge: u32) -> DrawRequest {
    let start = meeting.start.with_timezone(&Local);
    let end = meeting.end.with_timezone(&Local);
    let today = now.with_timezone(&Local).date_naive();
    let to_start = (meeting.start - now).num_seconds();
    let subject = transliterate_subject(&meeting.subject, 60);
    let start_label = if start.date_naive() == today {
        hhmm(start)
    } else {
        format!("{} {}", weekday_fr(start), hhmm(start))
    };

    let background = kind.background();
    let mut elements = vec![
        DrawElement::Rectangle {
            id: "bg".into(),
            timeout: ELEMENT_TIMEOUT_S,
            x: 0,
            y: 0,
            display: "front".into(),
            width: 72,
            height: 16,
            fill: if background.len() > 1 {
                "gradient_v"
            } else {
                "solid"
            }
            .into(),
            fill_colors: background.iter().map(|c| c.to_string()).collect(),
            border_width: 0,
        },
        DrawElement::Image {
            id: "ico".into(),
            timeout: ELEMENT_TIMEOUT_S,
            x: 1,
            y: 1,
            align: "top_left".into(),
            display: "front".into(),
            path: ICON_FILE.into(),
        },
        text(
            "badge",
            badge.min(99).to_string(),
            "superscript",
            8,
            6,
            "top_mid",
            "#323232FF",
        ),
        scrolling(
            "title",
            subject.clone(),
            "small",
            "front",
            18,
            0,
            54,
            if kind == ScreenKind::Idle { DIM } else { WHITE },
        ),
    ];
    let bottom_left =
        |value: String, color: &str| text("bl", value, "small", 18, 8, "top_left", color);
    let bottom_right = |value: &str, color: &str| {
        text("br", value.to_string(), "small", 72, 8, "top_right", color)
    };

    let (back_label, back_detail) = match kind {
        ScreenKind::Idle => {
            elements.push(bottom_left(start_label.clone(), DIM));
            // Not today: the weekday already fills the line.
            if start.date_naive() == today {
                elements.push(bottom_right(&remaining_label(to_start), WHITE));
            }
            (
                "PROCHAIN",
                format!("{start_label} - dans {}", remaining_label(to_start)),
            )
        }
        ScreenKind::Approach => {
            elements.push(bottom_left(hhmm(start), DIM));
            elements.push(bottom_right(&remaining_label(to_start), AMBER));
            (
                "BIENTOT",
                format!("{} - dans {}", hhmm(start), remaining_label(to_start)),
            )
        }
        ScreenKind::Imminent => {
            elements.push(countdown(meeting.start.timestamp(), "time_left", WHITE, 18));
            elements.push(bottom_right("IN", RED));
            ("MAINTENANT", format!("debut a {}", hhmm(start)))
        }
        ScreenKind::Armed => {
            elements.push(bottom_left(hhmm(start), DIM));
            elements.push(bottom_right("START", RED));
            (
                "C'EST L'HEURE",
                format!("{} - appuyer sur START", hhmm(start)),
            )
        }
        ScreenKind::InMeeting => {
            elements.push(countdown(meeting.end.timestamp(), "time_left", WHITE, 18));
            elements.push(bottom_right("LIVE", GREEN));
            ("EN REUNION", format!("fin a {}", hhmm(end)))
        }
        ScreenKind::Overrun => {
            elements.push(bottom_left("+".into(), ORANGE));
            elements.push(countdown(meeting.end.timestamp(), "time_since", ORANGE, 24));
            elements.push(bottom_right("OVER", ORANGE));
            ("DEPASSEMENT", format!("fin prevue a {}", hhmm(end)))
        }
    };
    elements.push(back_line("back-title", back_label.into(), "small", 8));
    elements.push(back_line("back-subject", subject, "normal", 35));
    elements.push(back_line("back-detail", back_detail, "small", 67));

    let led = match kind {
        ScreenKind::Imminent if to_start <= 60 => Some("#E24B4AFF"),
        ScreenKind::Armed => Some("#FF0000FF"),
        ScreenKind::Overrun => Some(ORANGE),
        _ => None,
    };
    DrawRequest {
        application_name: APP.to_string(),
        priority: kind.priority(),
        // A tentative meeting never blinks the LED.
        led_notification_color: led
            .filter(|_| meeting.show_as != "tentative")
            .map(str::to_string),
        elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn meeting(start: DateTime<Utc>) -> Meeting {
        Meeting {
            id: "m1".into(),
            subject: "Réunion équipe".into(),
            start,
            end: start + Duration::minutes(30),
            is_online: false,
            join_url: None,
            show_as: "busy".into(),
            response: "none".into(),
            is_cancelled: false,
            is_all_day: false,
        }
    }

    fn json(kind: ScreenKind, m: &Meeting, now: DateTime<Utc>) -> serde_json::Value {
        serde_json::to_value(build(kind, m, now, 3)).expect("serialize")
    }

    fn element<'a>(value: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
        value["elements"]
            .as_array()
            .expect("elements")
            .iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| panic!("element {id} missing"))
    }

    #[test]
    fn sounds_follow_the_scene_changes() {
        assert_eq!(entry_sound(ScreenKind::Idle), None);
        assert_eq!(
            entry_sound(ScreenKind::Approach),
            Some(Sound::Asset("doux.snd"))
        );
        assert!(matches!(
            entry_sound(ScreenKind::Imminent),
            Some(Sound::Stock(_))
        ));
        assert!(matches!(
            entry_sound(ScreenKind::Armed),
            Some(Sound::Stock(_))
        ));
        // The end-of-meeting chime is tied to the LIVE state, not to a screen.
        assert_eq!(entry_sound(ScreenKind::InMeeting), None);
        assert_eq!(entry_sound(ScreenKind::Overrun), None);
        // Every custom sound we may play is shipped in the uploaded assets.
        for sound in [
            entry_sound(ScreenKind::Approach).unwrap(),
            MEETING_END_SOUND,
        ] {
            if let Sound::Asset(file) = sound {
                assert!(ASSETS
                    .iter()
                    .any(|(name, bytes)| *name == file && !bytes.is_empty()));
            }
        }
    }

    #[test]
    fn kind_follows_time_before_start() {
        let now = Utc::now();
        let at = |minutes: i64| {
            ScreenKind::before_start(&meeting(now + Duration::minutes(minutes)), now)
        };
        assert_eq!(at(120), ScreenKind::Idle);
        assert_eq!(at(16), ScreenKind::Idle);
        assert_eq!(at(15), ScreenKind::Approach);
        assert_eq!(at(6), ScreenKind::Approach);
        assert_eq!(at(5), ScreenKind::Imminent);
        assert_eq!(at(0), ScreenKind::Imminent);
    }

    #[test]
    fn remaining_label_rounds_up_and_switches_to_hours() {
        assert_eq!(remaining_label(12 * 60), "12 min");
        assert_eq!(remaining_label(11 * 60 + 1), "12 min");
        assert_eq!(remaining_label(59 * 60 + 30), "1h00");
        assert_eq!(remaining_label(125 * 60), "2h05");
        assert_eq!(remaining_label(-5), "0 min");
    }

    #[test]
    fn every_screen_shares_the_template() {
        let now = Utc::now();
        let m = meeting(now + Duration::minutes(3));
        for kind in [
            ScreenKind::Idle,
            ScreenKind::Approach,
            ScreenKind::Imminent,
            ScreenKind::Armed,
            ScreenKind::InMeeting,
            ScreenKind::Overrun,
        ] {
            let value = json(kind, &m, now);
            assert_eq!(value["application_name"], APP);
            assert_eq!(element(&value, "ico")["path"], ICON_FILE);
            assert_eq!(element(&value, "badge")["text"], "3");
            let title = element(&value, "title");
            assert_eq!(title["text"], "Reunion equipe");
            assert_eq!(title["x"], 18);
            assert_eq!(title["width"], 54);
            assert_eq!(element(&value, "back-subject")["display"], "back");
            for e in value["elements"].as_array().unwrap() {
                assert_eq!(e["timeout"], ELEMENT_TIMEOUT_S, "{kind:?} {e}");
                if let Some(t) = e["text"].as_str() {
                    assert!(t.is_ascii(), "{kind:?}: non-ASCII text {t}");
                }
            }
        }
    }

    #[test]
    fn bottom_line_matches_each_state() {
        let now = Utc::now();
        let soon = meeting(now + Duration::minutes(12));
        let approach = json(ScreenKind::Approach, &soon, now);
        assert_eq!(element(&approach, "br")["text"], "12 min");
        // Above a running JS app (50), below the attention screens (65).
        assert_eq!(approach["priority"], 56);

        let imminent = json(
            ScreenKind::Imminent,
            &meeting(now + Duration::minutes(3)),
            now,
        );
        assert_eq!(element(&imminent, "cd")["direction"], "time_left");
        assert_eq!(element(&imminent, "br")["text"], "IN");

        let started = meeting(now - Duration::minutes(2));
        assert_eq!(
            element(&json(ScreenKind::Armed, &started, now), "br")["text"],
            "START"
        );
        let live = json(ScreenKind::InMeeting, &started, now);
        assert_eq!(
            element(&live, "cd")["timestamp"],
            started.end.timestamp().to_string()
        );
        assert_eq!(element(&live, "br")["text"], "LIVE");

        let over = json(
            ScreenKind::Overrun,
            &meeting(now - Duration::minutes(40)),
            now,
        );
        assert_eq!(element(&over, "cd")["direction"], "time_since");
        assert_eq!(element(&over, "cd")["x"], 24);
    }

    #[test]
    fn led_only_when_attention_is_needed() {
        let now = Utc::now();
        let led = |kind, m: &Meeting| build(kind, m, now, 1).led_notification_color;
        let far = meeting(now + Duration::hours(2));
        assert_eq!(led(ScreenKind::Idle, &far), None);
        assert_eq!(
            led(ScreenKind::Approach, &meeting(now + Duration::minutes(10))),
            None
        );
        assert_eq!(
            led(ScreenKind::Imminent, &meeting(now + Duration::minutes(3))),
            None
        );
        assert!(led(ScreenKind::Imminent, &meeting(now + Duration::seconds(45))).is_some());
        let started = meeting(now - Duration::minutes(1));
        assert_eq!(
            led(ScreenKind::Armed, &started).as_deref(),
            Some("#FF0000FF")
        );
        assert_eq!(led(ScreenKind::InMeeting, &started), None);
        let mut tentative = started.clone();
        tentative.show_as = "tentative".into();
        assert_eq!(led(ScreenKind::Armed, &tentative), None);
    }

    /// Plays the six screens on a real bar through the real client.
    /// Run with: `cargo test real_device_screens -- --ignored --nocapture`
    /// Optional: BUSYBAR_URL (default http://10.0.4.20), BUSYBAR_HOLD_S (default 3),
    /// BUSYBAR_SHOTS (folder receiving one front capture per screen).
    #[tokio::test]
    #[ignore]
    async fn real_device_screens() {
        use crate::busybar::BusyBarClient;
        let base = std::env::var("BUSYBAR_URL").unwrap_or_else(|_| "http://10.0.4.20".into());
        let hold: u64 = std::env::var("BUSYBAR_HOLD_S")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);
        let shots = std::env::var("BUSYBAR_SHOTS").ok();
        let client = BusyBarClient::new(&base).expect("client");
        client
            .upload_asset(APP, ICON_FILE, ICON_BYTES)
            .await
            .expect("icon upload");
        let now = Utc::now();
        let cases = [
            (
                "1-repos",
                ScreenKind::Idle,
                meeting(now + Duration::minutes(125)),
            ),
            (
                "2-approche",
                ScreenKind::Approach,
                meeting(now + Duration::minutes(12)),
            ),
            (
                "3-imminent",
                ScreenKind::Imminent,
                meeting(now + Duration::seconds(50)),
            ),
            (
                "4a-debut",
                ScreenKind::Armed,
                meeting(now - Duration::seconds(20)),
            ),
            (
                "4b-en-reunion",
                ScreenKind::InMeeting,
                meeting(now - Duration::minutes(5)),
            ),
            (
                "5-depassement",
                ScreenKind::Overrun,
                meeting(now - Duration::minutes(32)),
            ),
        ];
        for (name, kind, m) in cases {
            let _ = client.clear(APP).await;
            let request = build(kind, &m, now, 3);
            let result = client.draw(&request).await;
            println!("{name}: priority {} -> {result:?}", request.priority);
            assert!(result.is_ok(), "{name} rejected by the bar");
            tokio::time::sleep(std::time::Duration::from_secs(hold.min(2))).await;
            if let Some(dir) = &shots {
                let body = reqwest::get(format!("{base}/api/screen?display=0"))
                    .await
                    .expect("screen")
                    .text()
                    .await
                    .expect("screen body");
                std::fs::write(format!("{dir}/{name}-avant.b64"), body).expect("write capture");
            }
            tokio::time::sleep(std::time::Duration::from_secs(hold.saturating_sub(2))).await;
        }
        client.clear(APP).await.expect("final clear");
    }

    #[test]
    fn meeting_on_another_day_shows_the_weekday() {
        let now = Utc::now();
        let m = meeting(now + Duration::days(2));
        let value = json(ScreenKind::Idle, &m, now);
        let left = element(&value, "bl")["text"].as_str().unwrap().to_string();
        assert_eq!(left.len(), "lun 12:30".len(), "got {left}");
        assert!(value["elements"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != "br"));
    }
}
