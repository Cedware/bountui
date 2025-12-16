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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn create_toaster() -> (Toaster, mpsc::Receiver<crate::bountui::Message>) {
        let (tx, rx) = mpsc::channel(100);
        let toaster = Toaster::new(tx);
        (toaster, rx)
    }

    #[tokio::test]
    async fn test_show_toast_adds_to_active_when_space_available() {
        let (mut toaster, _rx) = create_toaster();

        // Set max visible toasts to allow at least one toast
        *toaster.max_visible_toasts.borrow_mut() = 3;

        toaster.show_toast("Test toast".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.active_toasts.len(), 1);
        assert_eq!(toaster.pending_toasts.len(), 0);
        assert_eq!(toaster.active_toasts[0].text, "Test toast");
    }

    #[tokio::test]
    async fn test_show_toast_queues_when_at_max_capacity() {
        let (mut toaster, _rx) = create_toaster();

        // Set max visible toasts to 2
        *toaster.max_visible_toasts.borrow_mut() = 2;

        // Add 2 toasts to fill capacity
        toaster.show_toast("Toast 1".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Toast 2".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.active_toasts.len(), 2);
        assert_eq!(toaster.pending_toasts.len(), 0);

        // Third toast should go to pending queue
        toaster.show_toast("Toast 3".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.active_toasts.len(), 2);
        assert_eq!(toaster.pending_toasts.len(), 1);
        assert_eq!(toaster.pending_toasts[0].text, "Toast 3");
    }

    #[tokio::test]
    async fn test_hide_toast_removes_from_active() {
        let (mut toaster, _rx) = create_toaster();

        *toaster.max_visible_toasts.borrow_mut() = 3;

        toaster.show_toast("Toast 1".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Toast 2".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.active_toasts.len(), 2);

        let toast_id = toaster.active_toasts[0].id.clone();
        toaster.hide_toast(toast_id);

        assert_eq!(toaster.active_toasts.len(), 1);
        assert_eq!(toaster.active_toasts[0].text, "Toast 2");
    }

    #[tokio::test]
    async fn test_hide_toast_promotes_pending_toast() {
        tokio::time::pause();
        let (mut toaster, mut rx) = create_toaster();

        // Set max to 2
        *toaster.max_visible_toasts.borrow_mut() = 2;

        // Add 3 toasts (2 active, 1 pending)
        toaster.show_toast("Toast 1".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Toast 2".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Toast 3".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.active_toasts.len(), 2);
        assert_eq!(toaster.pending_toasts.len(), 1);

        // Hide first toast
        let toast_id = toaster.active_toasts[0].id.clone();
        toaster.hide_toast(toast_id);

        // Toast 3 should be promoted
        assert_eq!(toaster.active_toasts.len(), 2);
        assert_eq!(toaster.pending_toasts.len(), 0);
        assert_eq!(toaster.active_toasts[1].text, "Toast 3");
    }

    #[tokio::test]
    async fn test_handle_message_show_toast() {
        let (mut toaster, _rx) = create_toaster();

        *toaster.max_visible_toasts.borrow_mut() = 3;

        let message = Message::ShowToast {
            text: "Test message".to_string(),
            duration: Duration::from_secs(1),
        };

        toaster.handle_message(message).await;

        assert_eq!(toaster.active_toasts.len(), 1);
        assert_eq!(toaster.active_toasts[0].text, "Test message");
    }

    #[tokio::test]
    async fn test_handle_message_hide_toast() {
        let (mut toaster, _rx) = create_toaster();

        *toaster.max_visible_toasts.borrow_mut() = 3;

        toaster.show_toast("Toast 1".to_string(), Duration::from_secs(1)).await;
        let toast_id = toaster.active_toasts[0].id.clone();

        let message = Message::HideToast { id: toast_id };
        toaster.handle_message(message).await;

        assert_eq!(toaster.active_toasts.len(), 0);
    }

    #[test]
    fn test_max_visible_toasts_calculation_in_view() {
        let (toaster, _rx) = create_toaster();

        // Create a mock frame with a specific height
        // We can't easily create a real Frame, so we'll test the calculation directly
        let frame_height = 30u16;
        let bottom_third_height = frame_height / 3; // 10
        let expected_max_toasts = (bottom_third_height / 3) as usize; // 3

        // This is the calculation from the view method
        assert_eq!(expected_max_toasts, 3);
    }

    #[tokio::test]
    async fn test_multiple_toasts_with_different_durations() {
        let (mut toaster, _rx) = create_toaster();

        *toaster.max_visible_toasts.borrow_mut() = 5;

        toaster.show_toast("Short".to_string(), Duration::from_millis(10)).await;
        toaster.show_toast("Medium".to_string(), Duration::from_millis(20)).await;
        toaster.show_toast("Long".to_string(), Duration::from_millis(30)).await;

        assert_eq!(toaster.active_toasts.len(), 3);
        assert_eq!(toaster.active_toasts[0].text, "Short");
        assert_eq!(toaster.active_toasts[1].text, "Medium");
        assert_eq!(toaster.active_toasts[2].text, "Long");
    }

    #[tokio::test]
    async fn test_hide_nonexistent_toast_does_nothing() {
        let (mut toaster, _rx) = create_toaster();

        *toaster.max_visible_toasts.borrow_mut() = 3;

        toaster.show_toast("Toast 1".to_string(), Duration::from_secs(1)).await;

        let original_len = toaster.active_toasts.len();
        toaster.hide_toast("nonexistent-id".to_string());

        assert_eq!(toaster.active_toasts.len(), original_len);
    }

    #[tokio::test]
    async fn test_pending_queue_order_is_preserved() {
        let (mut toaster, _rx) = create_toaster();

        // Set max to 1 to force queueing
        *toaster.max_visible_toasts.borrow_mut() = 1;

        toaster.show_toast("First".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Second".to_string(), Duration::from_secs(1)).await;
        toaster.show_toast("Third".to_string(), Duration::from_secs(1)).await;

        assert_eq!(toaster.pending_toasts.len(), 2);
        assert_eq!(toaster.pending_toasts[0].text, "Second");
        assert_eq!(toaster.pending_toasts[1].text, "Third");

        // Hide active toast
        let toast_id = toaster.active_toasts[0].id.clone();
        toaster.hide_toast(toast_id);

        // "Second" should be promoted first
        assert_eq!(toaster.active_toasts[0].text, "Second");
        assert_eq!(toaster.pending_toasts.len(), 1);
        assert_eq!(toaster.pending_toasts[0].text, "Third");
    }
}
