use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use crate::appframework::{Component, UpdateState};
use crate::bountui::components::input_dialog::InputDialogMessage::{SelectItem, UpdateInput};
use crate::bountui::Message;

#[derive(Debug, Clone)]
pub enum InputDialogMessage<InputId> {
    SelectItem(SelectedItem),
    UpdateInput(InputId, Event)
}

#[derive(Debug)]
pub struct InputField<InputId>
{
    pub id: InputId,
    pub title: String,
    pub value: Input,
}

pub struct Button<M, InputId>
{
    title: String,
    on_triggered: Box<dyn Fn(&Vec<InputField<InputId>>) -> M>,
}

impl<M, InputId> Button<M,InputId>
{
    pub fn new<T>(title: T, on_pressed: impl Fn(&Vec<InputField<InputId>>) -> M + 'static) -> Self
    where
        T: Into<String>,
    {
        Self {
            title: title.into(),
            on_triggered: Box::new(on_pressed),
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

#[derive(Debug, Clone)]
pub enum SelectedItem {
    Field(usize),
    Button(usize),
}

impl SelectedItem {
    
    fn is_field(&self) -> bool {
        matches!(self, SelectedItem::Field(_))
    }
}

pub struct InputDialog<FieldId, M>
{
    title: String,
    fields: Vec<InputField<FieldId>>,
    buttons: Vec<Button<M, FieldId>>,
    width: Constraint,
    height: Constraint,
    selected_item: SelectedItem,
}

impl<FieldId, M> InputDialog<FieldId, M>

{
    pub fn new(
        title: &str,
        fields: Vec<InputField<FieldId>>,
        buttons: Vec<Button<M,FieldId>>,
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
    
}

impl<FieldId, M> InputDialog<FieldId, M> where FieldId: Clone, M: From<InputDialogMessage<FieldId>>
{
    fn handle_event_while_input_selected(&self, event: &Event, selected_input_index: usize) -> Option<M> {
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Up => {
                    if selected_input_index > 0 {
                        return Some(M::from(SelectItem(SelectedItem::Field(selected_input_index - 1))))
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    return if selected_input_index < self.fields.len() - 1 {
                        M::from(SelectItem(SelectedItem::Field(selected_input_index + 1))).into()
                    } else {
                        M::from(SelectItem(SelectedItem::Button(0))).into()
                    }
                }
                _ => {
                    if let Some(input) = self.fields.get(selected_input_index) {
                        return M::from(UpdateInput(input.id.clone(), event.clone())).into();
                    }
                }
            }
        }
        None
    }

    fn handle_event_while_button_is_selected(
        &self,
        event: &Event,
        selected_button_index: usize,
    ) -> Option<M> {
        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Up => {
                    return M::from(SelectItem(SelectedItem::Field(self.fields.len() - 1))).into();
                }
                KeyCode::Left => {
                    if selected_button_index > 0 {
                        return M::from(SelectItem(SelectedItem::Button(selected_button_index - 1))).into();
                    }
                }
                KeyCode::Right => {
                    if selected_button_index < self.buttons.len() - 1 {
                        return M::from(SelectItem(SelectedItem::Button(selected_button_index + 1))).into()
                    }
                }
                KeyCode::Enter => {
                    let button = self.buttons.get(selected_button_index).unwrap();
                    return (button.on_triggered)(&self.fields).into();
                }
                KeyCode::Tab => {
                    if selected_button_index < self.buttons.len() - 1 {
                        return M::from(SelectItem(SelectedItem::Button(selected_button_index + 1))).into();
                    } else {
                        return M::from(SelectItem(SelectedItem::Field(0))).into();
                    }
                }
                _ => {}
            }
        }
        None
    }


    fn update_input(&mut self, field_id: &FieldId, event: &Event) where FieldId: Eq {
        if let Some(input) = self.fields.iter_mut().find(|field| field.id == *field_id) {
            input.value.handle_event(event);
        }
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


}

impl <FieldId, M> Component<M> for InputDialog<FieldId, M> where M: From<InputDialogMessage<FieldId>> , FieldId: Clone {
    fn view(&self, frame: &mut Frame) {
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

    fn handle_event(&self, event: &Event) -> Option<M> {

        match self.selected_item {
            SelectedItem::Field(i) => self.handle_event_while_input_selected(event, i),
            SelectedItem::Button(i) => self.handle_event_while_button_is_selected(event, i),
        }

    }
}

impl <FieldId, M> UpdateState<InputDialogMessage<FieldId>, Message> for InputDialog<FieldId, M> where FieldId: Eq + Clone, M: From<InputDialogMessage<FieldId>> {
    async fn update(&mut self, message: &InputDialogMessage<FieldId>) -> Option<Message> {
        match message {
            SelectItem(selected_item) => {
                self.selected_item = selected_item.clone();
            },
            UpdateInput(field_id, event) => {
                self.update_input(field_id, event);
            }
        }
        None
    }
}