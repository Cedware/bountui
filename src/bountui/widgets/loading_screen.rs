use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::prelude::{Line, Stylize, Widget};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct LoadingScreen {
    pub frame_count: u64,
}

impl Widget for LoadingScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([Constraint::Percentage(50)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(50)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center);

        let [_, text_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
            .areas(block.inner(area));

        let spinner = SPINNER_FRAMES[self.frame_count as usize % SPINNER_FRAMES.len()];

        let message = format!("{spinner} Loading...");
        let paragraph = Paragraph::new(Line::from(message))
            .alignment(Alignment::Center)
            .wrap(Wrap::default());

        Clear.render(area, buf);
        block.render(area, buf);
        paragraph.render(text_area, buf);
    }
}
