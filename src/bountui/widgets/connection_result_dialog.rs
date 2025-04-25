use crate::boundary;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};

pub struct ConnectResponseDialog<'a> {
    connect_result: &'a boundary::ConnectResponse,
}

impl<'a> ConnectResponseDialog<'a> {
    pub fn new(connect_result: &'a boundary::ConnectResponse) -> Self {
        Self { connect_result }
    }
}

impl<'a> Widget for ConnectResponseDialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let vertical = Layout::vertical([Constraint::Percentage(50)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(50)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let title = "Success";

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center)
            .title(title);
        let inner_area = block.inner(area);
        let [text_area, _, button_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner_area);

        let message = "Connection established".into();

        let credentials = self
            .connect_result
            .credentials
            .iter()
            .map(|c| format!("{}: {}", c.credential.username, c.credential.password).into())
            .collect::<Vec<Line>>();

        let mut lines = vec![Line::raw(""), message];

        for credential in credentials {
            lines.push(Line::raw(""));
            lines.push(credential);
        }

        let paragraph = Paragraph::new(lines).alignment(Alignment::Center);

        let ok_button = Span::from("    Ok    ").bold().reversed();
        let button_paragraph = Paragraph::new(Line::from(ok_button)).alignment(Alignment::Center);

        Clear.render(area, buf);
        block.render(area, buf);
        paragraph.render(text_area, buf);
        button_paragraph.render(button_area, buf);
    }
}
