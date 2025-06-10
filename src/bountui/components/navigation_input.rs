use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Stylize};
use ratatui::widgets::{Block, Paragraph};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use crate::bountui::Message;

const SCOPE_TREE: &str = "scope-tree";
const MY_SESSIONS: &str = "my-sessions";

pub struct NavigationInput {
    pub input: Input,
    pub message_tx: tokio::sync::mpsc::Sender<Message>,
}

impl NavigationInput {
    pub fn new(message_tx: tokio::sync::mpsc::Sender<Message>) -> Self {
        NavigationInput {
            input: Input::default(),
            message_tx
        }
    }

    async fn handle_confirm(&self) {
        match self.input.value() {
            SCOPE_TREE => {
                self.message_tx.send(Message::NavigateToScopeTree).await.unwrap();
            },
            MY_SESSIONS => {
                self.message_tx.send(Message::NavigateToMySessions).await.unwrap();
            },
            _ => {}
        }
    }

    pub async fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Enter {
                self.handle_confirm().await;
                return;
            }
        }
        self.input.handle_event(event);
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered().cyan().on_black();
        let paragraph = Paragraph::new(format!("> {}", self.input.value()))
            .block(block)
            .alignment(Alignment::Left);
        frame.render_widget(paragraph, area);

    }
}