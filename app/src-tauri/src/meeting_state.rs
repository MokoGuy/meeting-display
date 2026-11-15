use crate::calendar::Meeting;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum MeetingState {
    #[default]
    Idle,
    Armed {
        meeting: Meeting,
    },
    InMeeting {
        meeting: Meeting,
        end: DateTime<Utc>,
        saved_snapshot: Option<Value>,
        started_at: DateTime<Utc>,
    },
    Overrun {
        meeting: Meeting,
        end: DateTime<Utc>,
        saved_snapshot: Option<Value>,
    },
}

/// Native BUSY timer payload. Unused while the "in meeting" screen is drawn by
/// the app; kept for a future option that starts the bar's own timer instead.
#[allow(dead_code)]
pub fn simple_snapshot(end: DateTime<Utc>, now: DateTime<Utc>, trigger_smart_home: bool) -> Value {
    json!({
        "snapshot": {
            "type": "SIMPLE",
            "card_id": "00000000-0000-0000-0000-000000000000",
            "time_left_ms": (end - now).num_milliseconds().max(0),
            "is_paused": false,
            "busy_bar_settings": {
                "theme": "meeting",
                "show_work_phase_only": true,
                "trigger_smart_home": trigger_smart_home
            }
        },
        "snapshot_timestamp_ms": now.timestamp_millis()
    })
}

pub fn restore_snapshot(saved: &Value, now: DateTime<Utc>, started_at: DateTime<Utc>) -> Value {
    let mut restored = saved.clone();
    if let Some(object) = restored.as_object_mut() {
        object.insert(
            "snapshot_timestamp_ms".into(),
            json!(now.timestamp_millis()),
        );
        if let Some(snapshot) = object.get_mut("snapshot").and_then(Value::as_object_mut) {
            if snapshot.get("type").and_then(Value::as_str) == Some("SIMPLE") {
                let old_time = snapshot
                    .get("time_left_ms")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let elapsed = (now - started_at).num_milliseconds().max(0);
                snapshot.insert("time_left_ms".into(), json!((old_time - elapsed).max(0)));
            }
        }
    }
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting() -> Meeting {
        let start = DateTime::parse_from_rfc3339("2030-01-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        Meeting {
            id: "m".into(),
            subject: "Meeting".into(),
            start,
            end: start + chrono::Duration::minutes(30),
            is_online: false,
            join_url: None,
            show_as: "busy".into(),
            response: "accepted".into(),
            is_cancelled: false,
            is_all_day: false,
        }
    }

    #[test]
    fn simple_body_is_native_timer_payload() {
        let m = meeting();
        let body = simple_snapshot(m.end, m.start, true);
        assert_eq!(body["snapshot"]["type"], "SIMPLE");
        assert_eq!(body["snapshot"]["time_left_ms"], 1_800_000);
        assert_eq!(body["snapshot"]["busy_bar_settings"]["theme"], "meeting");
    }

    #[test]
    fn restore_recalculates_timestamp_and_remaining_time() {
        let m = meeting();
        let saved = simple_snapshot(m.end, m.start, false);
        let restored = restore_snapshot(&saved, m.start + chrono::Duration::seconds(10), m.start);
        assert_eq!(
            restored["snapshot_timestamp_ms"],
            (m.start + chrono::Duration::seconds(10)).timestamp_millis()
        );
        assert_eq!(restored["snapshot"]["time_left_ms"], 1_790_000);
    }
}
