use crate::app::UpdateResult;
use crate::components::Alert;
use crossterm::event::Event;
use ratatui::Frame;
use std::cell::RefCell;

#[derive(Default)]
pub struct Alerts {
    alerts: RefCell<Vec<Alert>>,
}

impl Alerts {
    pub fn alert<T: Into<String>, M: Into<String>>(&self, title: T, message: M) {
        self.alerts
            .borrow_mut()
            .push(Alert::new(title.into(), message.into()));
    }

    pub fn handle_event(&self, event: &Event) -> UpdateResult<()> {
        if self.alerts.borrow_mut().is_empty() {
            UpdateResult::NotHandled
        } else {
            if let Event::Key(key_event) = event {
                if key_event.code == crossterm::event::KeyCode::Enter {
                    self.alerts.borrow_mut().pop();
                }
            }
            UpdateResult::Handled(())
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        if let Some(alert) = self.alerts.borrow().first() {
            alert.render(frame);
        }
    }
}
