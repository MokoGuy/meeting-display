use crate::busybar::BusyBarClient;
use crate::state::AppState;
use crate::tray;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{interval, Duration};

pub async fn run(app: AppHandle) {
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;
        let state = app.state::<AppState>();
        let client = match BusyBarClient::new(&state.base_url()) {
            Ok(client) => client,
            Err(error) => {
                eprintln!("Could not create Busy Bar client: {error}");
                state.set_offline();
                tray::refresh(&app, false, None);
                continue;
            }
        };
        match client.status().await {
            Ok(status) => {
                let name = status.name.clone();
                state.set_connected(status);
                tray::refresh(&app, true, name.as_deref());
                if let Err(error) = app.emit("connection-changed", state.dto()) {
                    eprintln!("Could not emit connection state: {error}");
                }
            }
            Err(error) => {
                eprintln!("Busy Bar status unavailable: {error}");
                state.set_offline();
                tray::refresh(&app, false, None);
                if let Err(error) = app.emit("connection-changed", state.dto()) {
                    eprintln!("Could not emit connection state: {error}");
                }
            }
        }
    }
}
