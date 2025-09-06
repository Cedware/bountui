use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use crate::bountui::Message;

const SCOPE_TREE: &str = "scope-tree";
const MY_SESSIONS: &str = "my-sessions";

const OPTIONS: [&'static str; 2] = [SCOPE_TREE, MY_SESSIONS];

pub struct NavigationInput {
    pub input: Input,
    // Cached matching option for current input value
    pub matching_option: Option<&'static str>,
    pub message_tx: tokio::sync::mpsc::Sender<Message>,
}

impl NavigationInput {
    pub fn new(message_tx: tokio::sync::mpsc::Sender<Message>) -> Self {
        NavigationInput {
            input: Input::default(),
            matching_option: None,
            message_tx
        }
    }

    fn compute_matching_option(value: &str) -> Option<&'static str> {
        if value.is_empty() {
            return None;
        }
        OPTIONS.iter()
            .find(|opt| opt.starts_with(value))
            .map(|opt| *opt)
    }

    fn recompute_matching_option(&mut self) {
        self.matching_option = Self::compute_matching_option(self.input.value());
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
            match key_event.code {
                KeyCode::Enter => {
                    self.handle_confirm().await;
                    return;
                }
                KeyCode::Tab => {
                    if let Some(opt) = self.matching_option {
                        self.input = Input::new(opt.to_string());
                        self.recompute_matching_option();
                    }
                    return;
                }
                _ => {}
            }
        }
        self.input.handle_event(event);
        self.recompute_matching_option();
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered().cyan().on_black();
        let inner_area = block.inner(area);
        let typed = self.input.value();
        let mut spans: Vec<Span> = vec![Span::raw("> "), Span::raw(typed.to_string())];
        if let Some(opt) = self.matching_option {
            if typed.len() < opt.len() {
                let rest = &opt[typed.len()..];
                spans.push(Span::raw(rest).dark_gray());
            }
        }
        let paragraph = Paragraph::new(Line::from(spans))
            .block(block)
            .alignment(Alignment::Left);
        frame.render_widget(paragraph, area);
        // Place cursor at the end of the typed text (not after the ghost completion)
        frame.set_cursor_position((
            inner_area.x + 2 + self.input.visual_cursor() as u16,
            inner_area.y,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key_char(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn key_tab() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }


    macro_rules! autocomplete_tests {
        ($($name:ident: ($typed:expr, $expected:expr),)*) => {
            $(
                #[tokio::test]
                async fn $name() {
                    let (tx, _rx) = tokio::sync::mpsc::channel(1);
                    let mut nav = NavigationInput::new(tx);

                    for c in $typed.chars() {
                        let e = key_char(c);
                        nav.handle_event(&e).await;
                    }
                    let tab = key_tab();
                    nav.handle_event(&tab).await;

                    assert_eq!(nav.input.value(), $expected);
                }
            )*
        }
    }

    autocomplete_tests! {
        autocomplete_accepts_scope_tree_on_tab: ("sco", "scope-tree"),
        autocomplete_accepts_my_sessions_on_tab: ("my-", "my-sessions"),
    }
}
