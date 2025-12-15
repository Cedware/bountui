use crate::bountui::widgets;
use ratatui::layout::{Constraint, Rect};
use ratatui::Frame;

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: String,
    pub text: String,
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
    toasts: Vec<Toast>,
    message_tx: tokio::sync::mpsc::Sender<crate::bountui::Message>,
}

impl Toaster {
    pub fn new(message_tx: tokio::sync::mpsc::Sender<crate::bountui::Message>) -> Self {
        Self {
            toasts: Vec::new(),
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
        self.toasts.push(Toast {
            id: id.clone(),
            text,
        });

        let message_tx = self.message_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = message_tx
                .send(crate::bountui::Message::Toaster(Message::HideToast { id }))
                .await;
        });
    }

    fn hide_toast(&mut self, id: String) {
        self.toasts.retain(|toast| toast.id != id);
    }

    pub fn view(&self, frame: &mut Frame) {
        // Render toasts overlaying the content at the bottom
        if !self.toasts.is_empty() {
            let toast_height = self.toasts.len() as u16 * 3;
            let frame_area = frame.area();

            // Position toasts at the bottom of the frame
            let toast_area = Rect {
                x: frame_area.x,
                y: frame_area.y + frame_area.height.saturating_sub(toast_height),
                width: frame_area.width,
                height: toast_height.min(frame_area.height),
            };

            let toast_constraints: Vec<Constraint> = self
                .toasts
                .iter()
                .map(|_| Constraint::Length(3))
                .collect();
            let toast_areas =
                ratatui::layout::Layout::vertical(toast_constraints).split(toast_area);

            for (i, toast) in self.toasts.iter().enumerate() {
                if i < toast_areas.len() {
                    frame.render_widget(widgets::Toast::new(toast.text.clone()), toast_areas[i]);
                }
            }
        }
    }
}
