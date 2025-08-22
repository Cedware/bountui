mod boundary;
mod bountui;
pub mod event_ext;
mod util;
mod cross_term;
#[cfg(test)]
mod mock;

use crate::boundary::ApiClient;
use crate::bountui::{BountuiApp, UserInputsPath};
use crate::cross_term::receive_cross_term_events;
use crate::util::clipboard::{ClipboardAccess, ArboardClipboard, NoopClipboard};
use std::env;
use std::rc::Rc;
use log::error;


#[tokio::main]
async fn main() {
    let boundary_client = boundary::CliClient::default();
    let connection_manager = bountui::connection_manager::DefaultConnectionManager::new(boundary_client.clone());
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

    let cross_term_event_rx = receive_cross_term_events();

    let clipboard: Box<dyn ClipboardAccess> = match ArboardClipboard::new() {
        Ok(c) => Box::new(c),
        Err(e) => {
            error!("Failed to initialize clipboard: {}. Falling back to NoopClipboard.", e);
            Box::new(NoopClipboard::default())
        }
    };

    let mut app = BountuiApp::new(
        boundary_client,
        auth_result.attributes.user_id,
        connection_manager,
        user_inputs_path,
        cross_term_event_rx,
        clipboard,
    ).await;
    let _ = app.run().await;
}
