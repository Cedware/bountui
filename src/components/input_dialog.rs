use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use std::hash::Hash;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

#[derive(Debug, Clone)]
pub struct InputField<Id>
where
    Id: Clone,
{
    id: Id,
    title: String,
    value: Input,
}

pub struct Button<ButtonId>
where
    ButtonId: Copy,
{
    id: ButtonId,
    title: String,
}

impl<ButtonId> Button<ButtonId>
where
    ButtonId: Copy,
{
    pub fn new<T>(id: ButtonId, title: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            id,
            title: title.into(),
        }
    }
}

impl<Id> InputField<Id>
where
    Id: Clone,
{
    pub fn new<T, V>(id: Id, title: T, value: V) -> Self
    where
        T: Into<String>,
        V: Into<String>,
    {
        Self {
            id,
            title: title.into(),
            value: Input::new(value.into()),
        }
    }
}

enum SelectedItem {
    Field(usize),
    Button(usize),
}

pub struct InputDialog<ButtonId, FieldId>
where
    ButtonId: Copy,
    FieldId: Clone,
{
    title: String,
    fields: Vec<InputField<FieldId>>,
    buttons: Vec<Button<ButtonId>>,
    width: Constraint,
    height: Constraint,
    selected_item: SelectedItem,
}

impl<ButtonId, FieldId> InputDialog<ButtonId, FieldId>
where
    ButtonId: Copy,
    FieldId: Copy + Eq + Hash,
{
    pub fn new(
        title: &str,
        fields: Vec<InputField<FieldId>>,
        buttons: Vec<Button<ButtonId>>,
    ) -> Self {
        let width = Constraint::Percentage(50);
        let height = Constraint::Percentage(50);
        Self {
            title: title.to_string(),
            selected_item: SelectedItem::Field(0),
            fields,
            buttons,
            width,
            height,
        }
    }

    pub fn value(&self, field_id: FieldId) -> Option<&str> {
        self.fields
            .iter()
            .find(|field| field.id == field_id)
            .map(|field| field.value.value())
    }
}

impl<ButtonId, FieldId> InputDialog<ButtonId, FieldId>
where
    ButtonId: Copy,
    FieldId: Clone,
{
    fn handle_event_while_input_selected(&mut self, event: &Event, selected_input_index: usize) {
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Up => {
                    if selected_input_index > 0 {
                        self.selected_item = SelectedItem::Field(selected_input_index - 1);
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    if selected_input_index < self.fields.len() - 1 {
                        self.selected_item = SelectedItem::Field(selected_input_index + 1);
                    } else {
                        self.selected_item = SelectedItem::Button(0);
                    }
                }
                _ => {
                    if let Some(input) = self.fields.get_mut(selected_input_index) {
                        input.value.handle_event(event);
                    }
                }
            }
        }
    }

    fn handle_event_while_button_is_selected(
        &mut self,
        event: &Event,
        selected_button_index: usize,
    ) -> Option<ButtonId> {
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Up => {
                    self.selected_item = SelectedItem::Field(self.fields.len() - 1);
                }
                KeyCode::Left => {
                    if selected_button_index > 0 {
                        self.selected_item = SelectedItem::Button(selected_button_index - 1);
                    }
                }
                KeyCode::Right => {
                    if selected_button_index < self.buttons.len() - 1 {
                        self.selected_item = SelectedItem::Button(selected_button_index + 1);
                    }
                }
                KeyCode::Enter => {
                    let button = self.buttons.get(selected_button_index).unwrap();
                    return Some(button.id);
                }
                KeyCode::Tab => {
                    self.selected_item = if selected_button_index < self.buttons.len() - 1 {
                        SelectedItem::Button(selected_button_index + 1)
                    } else {
                        SelectedItem::Field(0)
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn inputs(&self, max_title_len: usize) -> Paragraph {
        let input_lines: Vec<Line> = self
            .fields
            .iter()
            .flat_map(|field| {
                let white_space = " ".repeat(max_title_len - field.title.len());
                vec![
                    Line::from(format!("{}:{} {}", field.title, white_space, field.value)).bold(),
                    Line::raw(""),
                ]
            })
            .collect();
        Paragraph::new(input_lines).alignment(Alignment::Left)
    }

    fn buttons(&self) -> Paragraph {
        let buttons: Vec<Span> = self
            .buttons
            .iter()
            .enumerate()
            .map(|(i, button)| {
                let mut span = Span::from(format!("    {}    ", button.title)).bold();
                if let SelectedItem::Button(selected_button) = self.selected_item {
                    if i == selected_button {
                        span = span.reversed()
                    }
                }
                span
            })
            .collect();
        Paragraph::new(Line::from(buttons)).alignment(Alignment::Center)
    }

    fn position_cursor(&self, frame: &mut Frame, area: &Rect, max_title_len: usize) {
        if let SelectedItem::Field(i) = self.selected_item {
            let selected_field = self.fields.get(i).unwrap();
            frame.set_cursor_position((
                area.x + max_title_len as u16 + 2 + selected_field.value.visual_cursor() as u16,
                area.y + i as u16 * 2,
            ));
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> Option<ButtonId> {
        match self.selected_item {
            SelectedItem::Field(i) => {
                self.handle_event_while_input_selected(event, i);
                None
            }
            SelectedItem::Button(i) => self.handle_event_while_button_is_selected(event, i),
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let vertical = Layout::vertical([self.height]).flex(Flex::Center);
        let horizontal = Layout::horizontal([self.width]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);

        let block = Block::bordered()
            .light_blue()
            .on_black()
            .title_alignment(Alignment::Center)
            .title(self.title.to_string());
        let inner_area = block.inner(area);

        let [input_area, _, button_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner_area);

        let max_title_len = self
            .fields
            .iter()
            .map(|field| field.title.len())
            .max()
            .unwrap();

        self.position_cursor(frame, &input_area, max_title_len);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(self.inputs(max_title_len), input_area);
        frame.render_widget(self.buttons(), button_area);
    }
}
