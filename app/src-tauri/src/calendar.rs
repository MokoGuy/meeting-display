use calcard::icalendar::{ICalendar, ICalendarProperty, ICalendarValue};
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use regex::Regex;
use serde::Serialize;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Meeting {
    pub id: String,
    pub subject: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub is_online: bool,
    pub join_url: Option<String>,
    pub show_as: String,
    pub response: String,
    pub is_cancelled: bool,
    pub is_all_day: bool,
}

pub trait Provider: Send + Sync {
    fn fetch<'a>(
        &'a self,
        now: DateTime<Utc>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<Meeting>, String>> + Send + 'a>,
    >;
}

#[derive(Clone)]
pub struct IcsProvider {
    url: String,
    http: reqwest::Client,
}

impl IcsProvider {
    pub fn new(url: &str) -> Result<Self, String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
        // Plain HTTP is accepted for this machine only (local test calendars).
        let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"));
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err("ICS URL must use HTTPS".to_string());
        }
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            url: url.to_string(),
            http,
        })
    }
}

impl Provider for IcsProvider {
    fn fetch<'a>(
        &'a self,
        now: DateTime<Utc>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<Meeting>, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            let started = std::time::Instant::now();
            let response = self
                .http
                .get(&self.url)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|e| e.to_string())?;
            let meetings = if status == 200 {
                parse_ics(&body, now)
            } else {
                Vec::new()
            };
            tracing::info!(
                ics_fetch = true,
                status,
                events = meetings.len(),
                duration_ms = started.elapsed().as_millis()
            );
            if status != 200 {
                return Err(format!("ICS server returned HTTP {status}"));
            }
            Ok(meetings)
        })
    }
}

pub async fn refresh(app: tauri::AppHandle) {
    let url = app.state::<crate::state::AppState>().ics_url();
    if url.trim().is_empty() {
        app.state::<crate::state::AppState>()
            .set_meetings(Vec::new());
        let _ = app.emit("meetings-changed", Vec::<Meeting>::new());
    } else if let Ok(provider) = IcsProvider::new(&url) {
        if let Ok(meetings) = provider.fetch(Utc::now()).await {
            app.state::<crate::state::AppState>()
                .set_meetings(meetings.clone());
            let _ = app.emit("meetings-changed", meetings);
        }
    }
}

pub async fn run_poller(app: tauri::AppHandle) {
    loop {
        refresh(app.clone()).await;
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
    }
}

const EXPANSION_LIMIT: usize = 100_000;
/// How far ahead meetings are kept, so the next one is known across a weekend.
const WINDOW_DAYS: i64 = 7;

/// Strictly validate a Teams join URL before we ever open it: the URL comes
/// from untrusted ICS content, so a substring match is not enough (it would
/// accept `https://evil.example/teams.microsoft.com/l/meetup-join`).
pub fn is_valid_teams_url(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            parsed.scheme() == "https"
                && parsed.host_str() == Some("teams.microsoft.com")
                && parsed.path().starts_with("/l/meetup-join")
        }
        Err(_) => false,
    }
}

fn parse_ics(body: &str, now: DateTime<Utc>) -> Vec<Meeting> {
    let ical = match ICalendar::parse(body) {
        Ok(calendar) => calendar,
        Err(_) => return Vec::new(),
    };
    let expanded = ical.expand_dates(Tz::UTC, EXPANSION_LIMIT);
    let window_start = now - Duration::hours(1);
    let window_end = now + Duration::days(WINDOW_DAYS);
    let teams = Regex::new(r"https://[^ \r\n<>]*teams\.microsoft\.com/l/meetup-join[^ \r\n<>]*")
        .expect("static regex");
    let mut meetings = Vec::new();
    for event in expanded.events {
        let Some(event) = event.try_into_date_time() else {
            continue;
        };
        if event.start.with_timezone(&Utc) < window_start
            || event.start.with_timezone(&Utc) > window_end
        {
            continue;
        }
        let Some(component) = ical.component_by_id(event.comp_id) else {
            continue;
        };
        if component.component_type != calcard::icalendar::ICalendarComponentType::VEvent {
            continue;
        }
        let value = |property: ICalendarProperty| -> Option<String> {
            component
                .property(&property)
                .and_then(|entry| entry.values.first())
                .and_then(ICalendarValue::as_text)
                .map(str::to_owned)
        };
        let subject = value(ICalendarProperty::Summary).unwrap_or_default();
        let description = value(ICalendarProperty::Description).unwrap_or_default();
        let location = value(ICalendarProperty::Location).unwrap_or_default();
        let teams_field = value(ICalendarProperty::Other(
            "X-MICROSOFT-SKYPETEAMSMEETINGURL".into(),
        ))
        .unwrap_or_default();
        let join_url = [
            description.as_str(),
            location.as_str(),
            teams_field.as_str(),
        ]
        .iter()
        .flat_map(|text| teams.find_iter(text))
        .map(|m| m.as_str().to_owned())
        .find(|candidate| is_valid_teams_url(candidate));
        let status = value(ICalendarProperty::Status).unwrap_or_default();
        let transp = value(ICalendarProperty::Transp).unwrap_or_default();
        let busy_status = value(ICalendarProperty::Other(
            "X-MICROSOFT-CDO-BUSYSTATUS".into(),
        ))
        .unwrap_or_default();
        let response = component
            .properties(&ICalendarProperty::Attendee)
            .filter_map(|entry| {
                entry.parameter(&calcard::icalendar::ICalendarParameterName::Partstat)
            })
            .find_map(|partstat| partstat.as_text().map(str::to_owned))
            .unwrap_or_else(|| "none".into());
        let is_all_day = component
            .property(&ICalendarProperty::Dtstart)
            .and_then(|entry| entry.parameter(&calcard::icalendar::ICalendarParameterName::Value))
            .and_then(|value| value.as_text())
            .is_some_and(|value| value.eq_ignore_ascii_case("DATE"));
        let start = event.start.with_timezone(&Utc);
        let end = event.end.with_timezone(&Utc);
        if is_all_day
            || status.eq_ignore_ascii_case("CANCELLED")
            || response.eq_ignore_ascii_case("DECLINED")
            || transp.eq_ignore_ascii_case("TRANSPARENT")
            || busy_status.eq_ignore_ascii_case("FREE")
        {
            continue;
        }
        meetings.push(Meeting {
            id: format!(
                "{}-{}",
                component.uid().unwrap_or("event"),
                start.timestamp()
            ),
            subject,
            start,
            end,
            is_online: join_url.is_some(),
            join_url,
            // Outlook publishes the real status in X-MICROSOFT-CDO-BUSYSTATUS;
            // TRANSP only says opaque/transparent.
            show_as: if busy_status.eq_ignore_ascii_case("TENTATIVE") {
                "tentative".into()
            } else if transp.is_empty() {
                "busy".into()
            } else {
                transp.to_lowercase()
            },
            response: response.to_lowercase(),
            is_cancelled: false,
            is_all_day,
        });
    }
    for error in &expanded.errors {
        tracing::debug!(
            ics_component = error.comp_id,
            "ignoring malformed calendar component"
        );
    }
    meetings.sort_by_key(|meeting| meeting.start);
    meetings
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = include_str!("../tests/fixtures/sample.ics");

    #[test]
    fn parses_filters_and_teams_link() {
        let now = DateTime::parse_from_rfc3339("2026-09-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let meetings = parse_ics(SAMPLE, now);
        assert!(meetings
            .iter()
            .any(|m| m.subject == "Team sync" && m.is_online));
        assert!(!meetings.iter().any(|m| m.subject == "Free event"));
    }

    #[test]
    fn expands_rrule_and_exdate() {
        let now = DateTime::parse_from_rfc3339("2026-09-03T11:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let meetings = parse_ics(SAMPLE, now);
        assert_eq!(
            meetings.iter().filter(|m| m.subject == "Recurring").count(),
            // DAILY;COUNT=4 minus one EXDATE, all inside the 7-day window.
            3
        );
        assert!(meetings.iter().any(|m| m.subject == "Monthly advanced"));
    }

    #[test]
    fn resolves_embedded_windows_timezone() {
        let now = DateTime::parse_from_rfc3339("2026-09-03T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let meetings = parse_ics(SAMPLE, now);
        let meeting = meetings
            .iter()
            .find(|m| m.subject == "Windows timezone event")
            .unwrap();
        assert_eq!(
            meeting.start,
            DateTime::parse_from_rfc3339("2026-09-03T08:30:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn excludes_allday_cancelled_declined_and_free() {
        let now = DateTime::parse_from_rfc3339("2026-09-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let meetings = parse_ics(SAMPLE, now);
        for excluded in ["Free event", "Cancelled", "All day event", "Declined event"] {
            assert!(
                !meetings.iter().any(|m| m.subject == excluded),
                "{excluded} should be filtered out"
            );
        }
        // Sorted by start ascending.
        assert!(meetings.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn rejects_untrusted_teams_url() {
        assert!(is_valid_teams_url(
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting"
        ));
        // http, wrong host, host-in-path, and subdomain spoofing are all rejected.
        assert!(!is_valid_teams_url(
            "http://teams.microsoft.com/l/meetup-join/x"
        ));
        assert!(!is_valid_teams_url(
            "https://evil.example/teams.microsoft.com/l/meetup-join"
        ));
        assert!(!is_valid_teams_url(
            "https://teams.microsoft.com.evil.com/l/meetup-join"
        ));
    }

    #[test]
    fn outlook_tentative_status_is_kept() {
        let ics = "BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:t@example.test
DTSTART:20260903T120000Z
DTEND:20260903T123000Z
SUMMARY:Maybe
TRANSP:OPAQUE
X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE
END:VEVENT
BEGIN:VEVENT
UID:w@example.test
DTSTART:20260908T120000Z
DTEND:20260908T123000Z
SUMMARY:Next week
TRANSP:OPAQUE
X-MICROSOFT-CDO-BUSYSTATUS:BUSY
END:VEVENT
END:VCALENDAR
";
        let now = DateTime::parse_from_rfc3339("2026-09-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let meetings = parse_ics(ics, now);
        let maybe = meetings
            .iter()
            .find(|m| m.subject == "Maybe")
            .expect("tentative meeting");
        assert_eq!(maybe.show_as, "tentative");
        // Five days ahead: visible thanks to the 7-day window.
        assert!(meetings
            .iter()
            .any(|m| m.subject == "Next week" && m.show_as == "opaque"));
    }

    /// Parses a real calendar export kept outside the repository.
    /// Run with: `BUSYBAR_ICS_FILE=<path> cargo test real_ics_file -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_ics_file() {
        let path = std::env::var("BUSYBAR_ICS_FILE").expect("set BUSYBAR_ICS_FILE");
        let body = std::fs::read_to_string(path).expect("readable ICS file");
        let now = std::env::var("BUSYBAR_ICS_NOW")
            .ok()
            .map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .expect("BUSYBAR_ICS_NOW must be RFC 3339")
                    .with_timezone(&Utc)
            })
            .unwrap_or_else(Utc::now);
        let started = std::time::Instant::now();
        let mut meetings = parse_ics(&body, now);
        let elapsed = started.elapsed();
        meetings.sort_by_key(|meeting| meeting.start);
        println!(
            "{} bytes parsed in {:?}, {} meeting(s) kept in the window",
            body.len(),
            elapsed,
            meetings.len()
        );
        for meeting in &meetings {
            println!(
                "{} -> {} | {} | show_as={} response={} online={}",
                meeting
                    .start
                    .with_timezone(&chrono::Local)
                    .format("%a %d/%m %H:%M"),
                meeting.end.with_timezone(&chrono::Local).format("%H:%M"),
                meeting.subject,
                meeting.show_as,
                meeting.response,
                meeting.is_online
            );
        }
    }
}
