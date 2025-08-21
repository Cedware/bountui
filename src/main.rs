mod boundary;
mod bountui;
pub mod event_ext;
mod util;
mod cross_term;

use crate::boundary::ApiClient;
use crate::bountui::{BountuiApp, UserInputsPath};
use std::env;
use std::fs;
use std::path::PathBuf;
use flexi_logger::LoggerHandle;
use anyhow::Context;

fn init_logger() -> anyhow::Result<LoggerHandle> {
    // Initialize logging with flexi_logger
    // - Daily rotated log files
    // - Keep 7 days of logs
    // - Default level: info; overridable via env var "LOG_LEVEL"
    let log_spec = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    // Determine log directory per OS
    let log_dir: PathBuf = if cfg!(target_os = "windows") {
        let appdata = env::var("APPDATA").context("Failed to determine APPDATA")?;
        let mut path = PathBuf::from(appdata);
        path.push("bountui");
        path.push("logs");
        path
    } else {
        let mut path = home::home_dir().context("Failed to determine home directory")?;
        path.push(".local");
        path.push("share");
        path.push("bountui");
        path.push("logs");
        path
    };

    // Ensure log directory exists
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create log directory at '{}'", log_dir.display()))?;

    // Configure logger from spec string
    let logger = flexi_logger::Logger::try_with_str(log_spec)
        .context("Failed to configure logger from LOG_LEVEL")?;

    // Start logger writing to the file
    let handle = logger
        .log_to_file(flexi_logger::FileSpec::default().directory(&log_dir))
        .rotate(
            flexi_logger::Criterion::Age(flexi_logger::Age::Day),
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(7),
        )
        .start()
        .context("Failed to initialize logger")?;

    Ok(handle)
}

#[tokio::main]
async fn main() {
    if let Err(e) = init_logger() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::ConnectionManager::new(boundary_client.clone());
    let auth_result = boundary_client.authenticate().await.unwrap();

    //This is safe because this is the only place we set the environment variable
    unsafe { env::set_var("BOUNDARY_TOKEN", auth_result.attributes.token) };


    let user_inputs_path_buf = home::home_dir().map(|mut path| {
        path.push(".bountui");
        path.push("user_inputs.json");
        path
    });
    let user_inputs_path = if let Some(path) = user_inputs_path_buf.as_ref() {
        Some(UserInputsPath(path))
    } else {
        None
    };

    let mut app = BountuiApp::new(boundary_client, auth_result.attributes.user_id, connection_manager, user_inputs_path).await;
    let _ = app.run().await;


}
