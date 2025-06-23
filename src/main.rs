mod boundary;
mod bountui;
pub mod event_ext;
mod util;
mod cross_term;

use crate::boundary::ApiClient;
use crate::bountui::BountuiApp;
use std::env;
use std::fs::create_dir_all;
use std::path::Path;

#[tokio::main]
async fn main() {
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::ConnectionManager::new(boundary_client.clone());
    let auth_result = boundary_client.authenticate().await.unwrap();

    //This is safe because this is the only place we set the environment variable
    unsafe { env::set_var("BOUNDARY_TOKEN", auth_result.attributes.token) };

    let user_input_file_path = Path::new("/home/cedrick/.bountui/user_inputs.json");
    create_dir_all(user_input_file_path.parent().unwrap()).unwrap();

    let mut app = BountuiApp::new(boundary_client, auth_result.attributes.user_id, connection_manager, &user_input_file_path).await;
    let _ = app.run().await;


}
