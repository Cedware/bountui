use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::prelude::{Line, Span, Stylize, Widget};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

pub struct LoginScreen;

impl Widget for LoginScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([Constraint::Percentage(50)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(50)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center)
            .title(Span::from(" Login ").bold());

        let [_, text_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(block.inner(area));

        let message =
            "Logging in...\nPlease follow the instructions in your browser\nto complete the login.";
        let lines: Vec<Line> = message.lines().map(Line::raw).collect();
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap::default());

        Clear.render(area, buf);
        block.render(area, buf);
        paragraph.render(text_area, buf);
    }
}
