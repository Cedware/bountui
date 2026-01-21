use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Flex, Rect};
use ratatui::prelude::{Line, Span, Stylize, Widget};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

const TOAST_BORDER_WIDTH: u16 = 1;
const TOAST_PADDING: u16 = 5;

pub struct Toast {
    text: String,
}

impl Toast {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

impl Widget for Toast {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let toast_width = (UnicodeWidthStr::width(self.text.as_str()) as u16 + TOAST_BORDER_WIDTH * 2 + TOAST_PADDING * 2).min(area.width);

        // Center the toast horizontally
        let horizontal = ratatui::layout::Layout::horizontal([ratatui::layout::Constraint::Length(toast_width)])
            .flex(Flex::Center);
        let [toast_area] = horizontal.areas(area);

        // Clear the toast area only
        Clear.render(toast_area, buf);

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center);
        
        let paragraph = Paragraph::new(Line::from(Span::from(self.text)))
            .alignment(Alignment::Center)
            .block(block);

        paragraph.render(toast_area, buf);
    }
}

