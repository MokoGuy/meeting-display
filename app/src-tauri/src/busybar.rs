use reqwest::{Client, Url};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Busy Bar unreachable: {0}")]
    Unreachable(String),
    #[error("Busy Bar returned HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("Invalid Busy Bar response: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub name: Option<String>,
    pub transport: Option<String>,
    pub firmware: String,
    pub battery_charge: u8,
    pub power_state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKey {
    Up,
    Down,
    Ok,
    Back,
    Start,
    Busy,
    Custom,
    Off,
    Apps,
    Settings,
}

impl InputKey {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "ok" => Ok(Self::Ok),
            "back" => Ok(Self::Back),
            "start" => Ok(Self::Start),
            "busy" => Ok(Self::Busy),
            "custom" => Ok(Self::Custom),
            "off" => Ok(Self::Off),
            "apps" => Ok(Self::Apps),
            "settings" => Ok(Self::Settings),
            _ => Err(format!("Unknown input key: {value}")),
        }
    }
}

#[derive(Clone)]
pub struct BusyBarClient {
    base: Url,
    http: Client,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DrawRequest {
    pub application_name: String,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led_notification_color: Option<String>,
    pub elements: Vec<DrawElement>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum DrawElement {
    #[serde(rename = "text")]
    Text {
        id: String,
        timeout: u32,
        x: i32,
        y: i32,
        align: String,
        display: String,
        text: String,
        font: String,
        color: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scroll_rate: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scroll_start_delay: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scroll_repeat_delay: Option<u32>,
    },
    #[serde(rename = "countdown")]
    Countdown {
        id: String,
        timeout: u32,
        x: i32,
        y: i32,
        align: String,
        display: String,
        timestamp: String,
        color: String,
        direction: String,
        show_hours: String,
    },
    #[serde(rename = "rectangle")]
    Rectangle {
        id: String,
        timeout: u32,
        x: i32,
        y: i32,
        display: String,
        width: u32,
        height: u32,
        fill: String,
        fill_colors: Vec<String>,
        border_width: u32,
    },
    /// Image stored in this application's assets on the bar (see `upload_asset`).
    #[serde(rename = "image")]
    Image {
        id: String,
        timeout: u32,
        x: i32,
        y: i32,
        align: String,
        display: String,
        path: String,
    },
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    firmware: Firmware,
    power: Power,
}

#[derive(Debug, Deserialize)]
struct Firmware {
    version: String,
}

#[derive(Debug, Deserialize)]
struct Power {
    state: String,
    battery_charge: u8,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Option<String>,
}

impl BusyBarClient {
    pub fn new(base: &str) -> Result<Self, Error> {
        let mut base = Url::parse(base).map_err(|error| Error::Decode(error.to_string()))?;
        if !matches!(base.scheme(), "http" | "https") || base.host_str().is_none() {
            return Err(Error::Decode("base URL must use http or https".to_string()));
        }
        base.set_query(None);
        base.set_fragment(None);
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(2))
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|error| Error::Unreachable(error.to_string()))?;
        Ok(Self { base, http })
    }

    pub async fn status(&self) -> Result<Status, Error> {
        let response: StatusResponse = self.get_json("/api/status").await?;
        let name = self.get_json::<NameResponse>("/api/name").await?.name;
        Ok(Status {
            name,
            transport: None,
            firmware: response.firmware.version,
            battery_charge: response.power.battery_charge,
            power_state: response.power.state,
        })
    }

    pub async fn snapshot(&self) -> Result<Value, Error> {
        self.get_json("/api/busy/snapshot").await
    }

    pub async fn put_snapshot(&self, body: &Value) -> Result<(), Error> {
        self.write_json("/api/busy/snapshot", reqwest::Method::PUT, Some(body))
            .await
    }

    pub async fn input(&self, key: InputKey) -> Result<(), Error> {
        let started = std::time::Instant::now();
        let key = match key {
            InputKey::Up => "up",
            InputKey::Down => "down",
            InputKey::Ok => "ok",
            InputKey::Back => "back",
            InputKey::Start => "start",
            InputKey::Busy => "busy",
            InputKey::Custom => "custom",
            InputKey::Off => "off",
            InputKey::Apps => "apps",
            InputKey::Settings => "settings",
        };
        let url = self.endpoint("/api/input");
        let response = self
            .http
            .post(url)
            .query(&[("key", key)])
            .send()
            .await
            .map_err(|error| {
                tracing::info!(
                    method = "POST",
                    path = "/api/input",
                    status = 0u16,
                    duration_ms = started.elapsed().as_millis()
                );
                Error::Unreachable(error.to_string())
            })?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            tracing::info!(
                method = "POST",
                path = "/api/input",
                status = status.as_u16(),
                duration_ms = started.elapsed().as_millis()
            );
            Error::Unreachable(error.to_string())
        })?;
        tracing::info!(
            method = "POST",
            path = "/api/input",
            status = status.as_u16(),
            duration_ms = started.elapsed().as_millis()
        );
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                message: error_message(&body),
            });
        }

        serde_json::from_str::<SuccessResponse>(&body)
            .and_then(|response| {
                if response.result == "OK" {
                    Ok(())
                } else {
                    Err(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "unexpected result",
                    )))
                }
            })
            .map_err(|error| Error::Decode(error.to_string()))
    }

    pub async fn draw(&self, request: &DrawRequest) -> Result<(), Error> {
        self.write_json("/api/display/draw", reqwest::Method::POST, Some(request))
            .await
    }

    /// Current audio volume of the bar (0-100).
    pub async fn volume(&self) -> Result<f64, Error> {
        let body: Value = self.get_json("/api/audio/volume").await?;
        body.get("volume")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::Decode("missing volume".to_string()))
    }

    /// Sets the audio volume (0-100) without the bar's own "volume changed" sound.
    pub async fn set_volume(&self, volume: f64) -> Result<(), Error> {
        let path = format!("/api/audio/volume?volume={volume}&silent=1");
        self.write_json(&path, reqwest::Method::POST, None::<&Value>)
            .await
    }

    /// Plays one of the bar's built-in sounds, e.g. `shared/calendar_event_starts.snd`.
    pub async fn play_stock_sound(
        &self,
        application_name: &str,
        stock_path: &str,
    ) -> Result<(), Error> {
        let body = serde_json::json!({
            "application_name": application_name,
            "stock_path": stock_path,
        });
        self.write_json("/api/audio/play", reqwest::Method::POST, Some(&body))
            .await
    }

    /// Plays a sound previously stored in the application's assets (see `upload_asset`).
    pub async fn play_asset_sound(&self, application_name: &str, path: &str) -> Result<(), Error> {
        let body = serde_json::json!({
            "application_name": application_name,
            "path": path,
        });
        self.write_json("/api/audio/play", reqwest::Method::POST, Some(&body))
            .await
    }

    /// Stores a file (PNG image, sound…) in the application's assets on the bar
    /// so that draw elements can reference it with `path`.
    pub async fn upload_asset(
        &self,
        application_name: &str,
        file: &str,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let started = std::time::Instant::now();
        let response = self
            .http
            .post(self.endpoint("/api/assets/upload"))
            .query(&[("application_name", application_name), ("file", file)])
            .header("Content-Type", "application/octet-stream")
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|error| {
                tracing::info!(
                    method = "POST",
                    path = "/api/assets/upload",
                    status = 0u16,
                    duration_ms = started.elapsed().as_millis()
                );
                Error::Unreachable(error.to_string())
            })?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::info!(
            method = "POST",
            path = "/api/assets/upload",
            status = status.as_u16(),
            duration_ms = started.elapsed().as_millis()
        );
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                message: error_message(&body),
            });
        }
        Ok(())
    }

    pub async fn clear(&self, application_name: &str) -> Result<(), Error> {
        let started = std::time::Instant::now();
        let response = self
            .http
            .delete(self.endpoint("/api/display/draw"))
            .query(&[("application_name", application_name)])
            .send()
            .await
            .map_err(|error| {
                tracing::info!(
                    method = "DELETE",
                    path = "/api/display/draw",
                    status = 0u16,
                    duration_ms = started.elapsed().as_millis()
                );
                Error::Unreachable(error.to_string())
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| Error::Unreachable(error.to_string()))?;
        tracing::info!(
            method = "DELETE",
            path = "/api/display/draw",
            status = status.as_u16(),
            duration_ms = started.elapsed().as_millis()
        );
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                message: error_message(&body),
            });
        }
        Ok(())
    }

    async fn write_json<T: Serialize>(
        &self,
        path: &str,
        method: reqwest::Method,
        payload: Option<&T>,
    ) -> Result<(), Error> {
        let started = std::time::Instant::now();
        let mut request = self.http.request(method.clone(), self.endpoint(path));
        if let Some(payload) = payload {
            request = request.json(payload);
        }
        let response = request.send().await.map_err(|error| {
                tracing::info!(method = %method, path, status = 0u16, duration_ms = started.elapsed().as_millis());
                Error::Unreachable(error.to_string())
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| Error::Unreachable(error.to_string()))?;
        tracing::info!(method = %method, path, status = status.as_u16(), duration_ms = started.elapsed().as_millis());
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                message: error_message(&body),
            });
        }
        Ok(())
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, Error> {
        let started = std::time::Instant::now();
        let response = self
            .http
            .get(self.endpoint(path))
            .send()
            .await
            .map_err(|error| {
                tracing::info!(
                    method = "GET",
                    path,
                    status = 0u16,
                    duration_ms = started.elapsed().as_millis()
                );
                Error::Unreachable(error.to_string())
            })?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            tracing::info!(
                method = "GET",
                path,
                status = status.as_u16(),
                duration_ms = started.elapsed().as_millis()
            );
            Error::Unreachable(error.to_string())
        })?;
        tracing::info!(
            method = "GET",
            path,
            status = status.as_u16(),
            duration_ms = started.elapsed().as_millis()
        );
        if !status.is_success() {
            return Err(Error::Http {
                status: status.as_u16(),
                message: error_message(&body),
            });
        }
        serde_json::from_str(&body).map_err(|error| Error::Decode(error.to_string()))
    }

    fn endpoint(&self, path: &str) -> Url {
        self.base
            .join(path.trim_start_matches('/'))
            .expect("validated path")
    }
}

#[derive(Debug, Deserialize)]
struct NameResponse {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SuccessResponse {
    result: String,
}

fn error_message(body: &str) -> String {
    serde_json::from_str::<ErrorResponse>(body)
        .ok()
        .and_then(|response| response.error)
        .unwrap_or_else(|| body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    const STATUS: &str =
        r#"{"firmware":{"version":"1.1.1"},"power":{"state":"charged","battery_charge":87}}"#;
    const NAME: &str = r#"{"name":"BUSY bar"}"#;

    async fn server(responses: Vec<(String, u16, String, bool)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let responses = Arc::new(Mutex::new(responses));
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0; 4096];
                let size = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..size]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let response = {
                    let mut responses = responses.lock().await;
                    responses
                        .iter()
                        .position(|(expected, _, _, _)| expected == path)
                        .map(|index| responses.remove(index))
                };
                if let Some((_, status, body, delayed)) = response {
                    if delayed {
                        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                    } else {
                        let headers = format!(
                            "HTTP/1.1 {status} Test\r\nContent-Length: {}   \r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream
                            .write_all(format!("{headers}{body}").as_bytes())
                            .await
                            .unwrap();
                    }
                }
            }
        });
        format!("http://{address}")
    }

    // Fake server that also records each request's first line (METHOD path ...),
    // so tests can assert the HTTP method used.
    async fn server_recording(
        responses: Vec<(String, u16, String)>,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let responses = Arc::new(Mutex::new(responses));
        let log = Arc::new(Mutex::new(Vec::<String>::new()));
        let log_writer = log.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut request = vec![0; 4096];
                let size = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..size]);
                let first_line = request.lines().next().unwrap_or("").to_string();
                // Record the whole request (start line + headers + body) so tests
                // can assert both the method/path and the JSON payload.
                log_writer.lock().await.push(request.to_string());
                let path = first_line.split_whitespace().nth(1).unwrap_or("/");
                let response = {
                    let mut responses = responses.lock().await;
                    responses
                        .iter()
                        .position(|(expected, _, _)| expected == path)
                        .map(|index| responses.remove(index))
                };
                if let Some((_, status, body)) = response {
                    let headers = format!(
                        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream
                        .write_all(format!("{headers}{body}").as_bytes())
                        .await
                        .unwrap();
                }
            }
        });
        (format!("http://{address}"), log)
    }

    #[tokio::test]
    async fn status_decodes_success() {
        let base = server(vec![
            ("/api/status".into(), 200, STATUS.into(), false),
            ("/api/name".into(), 200, NAME.into(), false),
        ])
        .await;
        let status = BusyBarClient::new(&base).unwrap().status().await.unwrap();
        assert_eq!(status.name.as_deref(), Some("BUSY bar"));
        assert_eq!(status.firmware, "1.1.1");
        assert_eq!(status.battery_charge, 87);
    }

    #[tokio::test]
    async fn http_error_uses_json_message() {
        let base = server(vec![(
            "/api/status".into(),
            409,
            r#"{"error":"busy"}"#.into(),
            false,
        )])
        .await;
        let error = BusyBarClient::new(&base)
            .unwrap()
            .status()
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Http { status: 409, message } if message == "busy"));
    }

    #[tokio::test]
    async fn malformed_json_is_decode_error() {
        let base = server(vec![("/api/status".into(), 200, "not-json".into(), false)]).await;
        let error = BusyBarClient::new(&base)
            .unwrap()
            .status()
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Decode(_)));
    }

    #[tokio::test]
    async fn content_length_with_trailing_spaces_decodes() {
        let base = server(vec![(
            "/api/busy/snapshot".into(),
            200,
            r#"{"type":"NOT_STARTED"}"#.into(),
            false,
        )])
        .await;
        let snapshot = BusyBarClient::new(&base).unwrap().snapshot().await.unwrap();
        assert_eq!(snapshot["type"], "NOT_STARTED");
    }

    #[tokio::test]
    async fn timeout_is_unreachable() {
        let base = server(vec![("/api/status".into(), 200, STATUS.into(), true)]).await;
        let error = BusyBarClient::new(&base)
            .unwrap()
            .status()
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Unreachable(_)));
    }

    #[test]
    fn parses_valid_and_unknown_input_keys() {
        assert_eq!(InputKey::parse("busy"), Ok(InputKey::Busy));
        assert_eq!(
            InputKey::parse("invalid"),
            Err("Unknown input key: invalid".to_string())
        );
    }

    #[tokio::test]
    async fn input_sends_new_key_to_fake_server() {
        let base = server(vec![(
            "/api/input?key=busy".into(),
            200,
            r#"{"result":"OK"}"#.into(),
            false,
        )])
        .await;
        BusyBarClient::new(&base)
            .unwrap()
            .input(InputKey::Busy)
            .await
            .unwrap();
    }

    fn sample_draw_request() -> DrawRequest {
        DrawRequest {
            application_name: "teams_meet".into(),
            priority: 60,
            led_notification_color: Some("#6264A7FF".into()),
            elements: vec![DrawElement::Countdown {
                id: "c".into(),
                timeout: 15,
                x: 36,
                y: 10,
                align: "top_mid".into(),
                display: "front".into(),
                timestamp: "1788000000".into(),
                color: "#FFFFFFFF".into(),
                direction: "time_left".into(),
                show_hours: "when_non_zero".into(),
            }],
        }
    }

    #[tokio::test]
    async fn draw_uses_post_and_accepts_200() {
        let (base, log) = server_recording(vec![(
            "/api/display/draw".into(),
            200,
            r#"{"result":"OK"}"#.into(),
        )])
        .await;
        BusyBarClient::new(&base)
            .unwrap()
            .draw(&sample_draw_request())
            .await
            .unwrap();
        assert!(log
            .lock()
            .await
            .iter()
            .any(|line| line.starts_with("POST /api/display/draw")));
    }

    #[tokio::test]
    async fn draw_priority_rejection_is_http_error() {
        let base = server(vec![(
            "/api/display/draw".into(),
            409,
            r#"{"error":"priority too low"}"#.into(),
            false,
        )])
        .await;
        let error = BusyBarClient::new(&base)
            .unwrap()
            .draw(&sample_draw_request())
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Http { status: 409, .. }));
    }

    #[tokio::test]
    async fn put_snapshot_uses_put_method() {
        let (base, log) = server_recording(vec![(
            "/api/busy/snapshot".into(),
            200,
            r#"{"result":"OK"}"#.into(),
        )])
        .await;
        let body = serde_json::json!({
            "snapshot": { "type": "SIMPLE", "card_id": "00000000-0000-0000-0000-000000000000",
                "time_left_ms": 1000, "is_paused": false,
                "busy_bar_settings": { "theme": "meeting", "show_work_phase_only": true, "trigger_smart_home": false } },
            "snapshot_timestamp_ms": 0
        });
        BusyBarClient::new(&base)
            .unwrap()
            .put_snapshot(&body)
            .await
            .unwrap();
        let requests = log.lock().await;
        assert!(requests
            .iter()
            .any(|req| req.starts_with("PUT /api/busy/snapshot")));
        // The transmitted body is the SIMPLE native-timer snapshot.
        assert!(requests
            .iter()
            .any(|req| req.contains("\"type\":\"SIMPLE\"")));
    }

    #[tokio::test]
    async fn clear_uses_delete_with_application_name() {
        let (base, log) = server_recording(vec![(
            "/api/display/draw?application_name=teams_meet".into(),
            200,
            r#"{"result":"OK"}"#.into(),
        )])
        .await;
        BusyBarClient::new(&base)
            .unwrap()
            .clear("teams_meet")
            .await
            .unwrap();
        assert!(log
            .lock()
            .await
            .iter()
            .any(|line| line.starts_with("DELETE /api/display/draw?application_name=teams_meet")));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn status_real_device() {
        let client = BusyBarClient::new("http://10.0.4.20").unwrap();
        let status = client.status().await.unwrap();
        let snapshot = client.snapshot().await.unwrap();
        assert!(!status.firmware.is_empty());
        assert!(snapshot.is_object());
    }

    /// Raises the volume, plays a built-in sound, restores the volume.
    /// Run with: `cargo test sound_real_device -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn sound_real_device() {
        let client = BusyBarClient::new("http://10.0.4.20").unwrap();
        let before = client.volume().await.unwrap();
        client.set_volume(50.0).await.unwrap();
        assert_eq!(client.volume().await.unwrap(), 50.0);
        client
            .play_stock_sound("teams_meet", "shared/calendar_event_starts.snd")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
        // Our own sounds: upload, then play each one.
        for (file, bytes) in crate::screens::ASSETS {
            client
                .upload_asset("teams_meet", file, bytes)
                .await
                .unwrap();
            if file.ends_with(".snd") {
                println!("playing {file}");
                client.play_asset_sound("teams_meet", file).await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
            }
        }
        client.set_volume(before).await.unwrap();
        let after = client.volume().await.unwrap();
        println!("volume before {before}, restored to {after}");
        assert_eq!(after, before);
    }
}
