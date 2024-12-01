use ratatui::layout::{Alignment, Constraint, Flex, Layout};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub struct Alert {
    title: String,
    message: String,
}

impl Alert {
    pub fn new<T, M>(title: T, message: M) -> Self
    where
        T: Into<String>,
        M: Into<String>,
    {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
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

        let paragraph = Paragraph::new(vec![Line::raw(self.message.as_str())])
            .alignment(Alignment::Center)
            .wrap(Wrap::default());

        let ok_buttons = Span::from("    Ok    ").bold().reversed();
        let button_paragraph = Paragraph::new(Line::from(ok_buttons)).alignment(Alignment::Center);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(paragraph, text_area);
        frame.render_widget(button_paragraph, button_area);
    }
}
