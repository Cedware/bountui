use crate::bountui::widgets;
use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: String,
    pub text: String,
    pub duration: std::time::Duration,
}

#[derive(Debug)]
pub enum Message {
    ShowToast {
        text: String,
        duration: std::time::Duration,
    },
    HideToast {
        id: String,
    },
}

pub struct Toaster {
    active_toasts: Vec<Toast>,
    pending_toasts: Vec<Toast>,
    max_visible_toasts: RefCell<usize>,
    message_tx: tokio::sync::mpsc::Sender<crate::bountui::Message>,
}

impl Toaster {
    pub fn new(message_tx: tokio::sync::mpsc::Sender<crate::bountui::Message>) -> Self {
        Self {
            active_toasts: Vec::new(),
            pending_toasts: Vec::new(),
            max_visible_toasts: RefCell::new(0),
            message_tx,
        }
    }

    pub async fn handle_message(&mut self, message: Message) {
        match message {
            Message::ShowToast { text, duration } => self.show_toast(text, duration).await,
            Message::HideToast { id } => self.hide_toast(id),
        }
    }

    async fn show_toast(&mut self, text: String, duration: std::time::Duration) {
        let id = uuid::Uuid::new_v4().to_string();
        let toast = Toast {
            id: id.clone(),
            text,
            duration,
        };

        let max_visible = *self.max_visible_toasts.borrow();
        if self.active_toasts.len() < max_visible {
            // Space available, add to active toasts and start timer
            self.active_toasts.push(toast);
            self.start_hide_timer(id, duration).await;
        } else {
            // No space, add to pending queue
            self.pending_toasts.push(toast);
        }
    }

    fn hide_toast(&mut self, id: String) {
        self.active_toasts.retain(|toast| toast.id != id);

        // If there are pending toasts, promote the first one to active
        if !self.pending_toasts.is_empty() {
            let toast = self.pending_toasts.remove(0);
            let toast_id = toast.id.clone();
            let toast_duration = toast.duration;
            self.active_toasts.push(toast);

            // Start timer for the newly activated toast
            let message_tx = self.message_tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(toast_duration).await;
                let _ = message_tx
                    .send(crate::bountui::Message::Toaster(Message::HideToast { id: toast_id }))
                    .await;
            });
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        let frame_area = frame.area();

        // Calculate max toasts that fit in bottom third of terminal
        // Each toast takes 3 lines of height
        let bottom_third_height = frame_area.height / 3;
        let max_toasts = (bottom_third_height / 3) as usize;

        // Update max_visible_toasts for use in show_toast
        *self.max_visible_toasts.borrow_mut() = max_toasts;

        // Render active toasts only
        if !self.active_toasts.is_empty() {
            let toast_count = self.active_toasts.len().min(max_toasts);
            let toast_height = toast_count as u16 * 3;

            // Position toasts at the bottom of the frame
            let toast_area = Rect {
                x: frame_area.x,
                y: frame_area.y + frame_area.height.saturating_sub(toast_height),
                width: frame_area.width,
                height: toast_height,
            };

            let toast_constraints: Vec<Constraint> = (0..toast_count)
                .map(|_| Constraint::Length(3))
                .collect();
            let toast_areas =
                ratatui::layout::Layout::vertical(toast_constraints).split(toast_area);

            for (i, toast) in self.active_toasts.iter().take(toast_count).enumerate() {
                if i < toast_areas.len() {
                    frame.render_widget(widgets::Toast::new(toast.text.clone()), toast_areas[i]);
                }
            }
        }
    }

    async fn start_hide_timer(&self, id: String, duration: std::time::Duration) {
        let message_tx = self.message_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = message_tx
                .send(crate::bountui::Message::Toaster(Message::HideToast { id }))
                .await;
        });
    }
}
