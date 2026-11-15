use std::fs;
use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<WorkerGuard, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;
    let file_appender = tracing_appender::rolling::daily(log_dir, "app.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with_writer(writer)
        .try_init()
        .map_err(|error| error.to_string())?;
    Ok(guard)
}
