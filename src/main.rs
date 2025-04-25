mod boundary;
mod ext;
mod bountui;
mod appframework;
pub mod event_ext;

use std::env;
use crate::appframework::Application;
use crate::boundary::ApiClient;
use crate::bountui::BountuiApp;

#[tokio::main]
async fn main() {
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::ConnectionManager::new(boundary_client.clone());
    let auth_result = boundary_client.authenticate().await.unwrap();
    env::set_var("BOUNDARY_TOKEN", auth_result.attributes.token);
    BountuiApp::new(boundary_client, connection_manager).await
        .run(None)
        .await.unwrap();
}