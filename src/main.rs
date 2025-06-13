mod boundary;
mod bountui;
pub mod event_ext;
mod util;
mod cross_term;

use std::env;
use crate::boundary::ApiClient;
use crate::bountui::BountuiApp;




#[tokio::main]
async fn main() {
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::ConnectionManager::new(boundary_client.clone());
    let auth_result = boundary_client.authenticate().await.unwrap();

    //This is safe because this is the only place we set the environment variable
    unsafe { env::set_var("BOUNDARY_TOKEN", auth_result.attributes.token) };

    let mut app = BountuiApp::new(boundary_client, auth_result.attributes.user_id, connection_manager).await;
    let _ = app.run().await;


}
