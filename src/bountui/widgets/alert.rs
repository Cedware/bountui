use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::prelude::{Line, Span, Stylize, Widget};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

pub struct Alert {
    title: String,
    message: String
}

impl Alert {
    pub fn new(title: String, message: String) -> Self {
        Self { title, message }
    }
}

impl Widget for Alert {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let vertical = Layout::vertical([Constraint::Percentage(25)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(25)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center)
            .title(Span::from(format!(" {} ", self.title)).bold());

        let [_, text_area, _, button_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
            .areas(block.inner(area));

        let lines: Vec<Line> = self.message.lines().map(Line::raw).collect();
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap::default());

        let ok_buttons = Span::from("    Ok    ").bold().reversed();
        let button_paragraph = Paragraph::new(Line::from(ok_buttons)).alignment(Alignment::Center);


        Clear.render(area, buf);
        block.render(area, buf);
        paragraph.render(text_area, buf);
        button_paragraph.render(button_area, buf);
    }
}