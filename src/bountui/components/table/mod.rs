mod action;
mod filter;
pub mod scope;
pub mod sessions;
pub mod target;

use std::cell::{Cell, RefCell};
use std::cmp::{max, min};
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};

use crate::bountui::components::table::filter::Filter;
use ratatui::text::{Line, Span};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::rc::Rc;
use ratatui::prelude::Rect;
use tokio::sync::mpsc;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use crate::bountui::Message;
use crate::bountui::Message::GoBack;
pub use action::Action;

pub trait SortItems<T> {
    fn sort(items: &mut Vec<Rc<T>>);
}

pub trait FilterItems<T> {
    fn match_str(value: &str, search: &str) -> bool {
        value.to_lowercase().contains(&search.to_lowercase())
    }

    fn matches(item: &T, search: &str) -> bool;
}

pub struct TableColumn<T> {
    header: String,
    width: Constraint,
    get_value: Box<dyn Fn(&T) -> String>,
}

impl<T> TableColumn<T> {
    pub fn new(header: String, width: Constraint, get_value: Box<dyn Fn(&T) -> String>) -> Self {
        TableColumn {
            header,
            width,
            get_value,
        }
    }
}

pub struct TablePage<T> {
    title: String,
    columns: Vec<TableColumn<T>>,
    items: Vec<Rc<T>>,
    visible_items: Vec<Rc<T>>,
    table_state: RefCell<TableState>,
    filter: Filter,
    message_tx: mpsc::Sender<Message>,
    actions: Vec<Action<T>>,
    page_size: Cell<usize>,
}
impl<T> TablePage<T> where Self: SortItems<T> {
    pub fn new(title: String, columns: Vec<TableColumn<T>>, items: Vec<T>, actions: Vec<Action<T>>, message_tx: mpsc::Sender<Message>) -> Self {
        let mut items: Vec<Rc<T>> = items.into_iter().map(Rc::new).collect();
        Self::sort(&mut items);
        let visible_items: Vec<Rc<T>> = items.iter().cloned().collect();
        let mut table_page = TablePage {
            title,
            columns,
            items,
            visible_items,
            table_state: RefCell::new(TableState::default()),
            filter: Filter::Disabled,
            actions,
            message_tx,
            page_size: Cell::new(0),
        };
        table_page.select_first_or_none();
        table_page
    }

    fn select_first_or_none(&mut self) {
        self.table_state.borrow_mut().select(if self.visible_items.is_empty() { None } else { Some(0) });
    }

    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items.into_iter().map(Rc::new).collect();
        Self::sort(&mut self.items);
        self.visible_items = self.items.iter().cloned().collect();
        let selected_optional = self.table_state.borrow().selected();
        if let Some(selected) = selected_optional {
            if selected >= self.items.len() {
                self.select_first_or_none();
            }
        }

    }

    pub fn selected_item(&self) -> Option<Rc<T>> {
        self.table_state.borrow_mut().selected()
            .map(|i| self.visible_items.get(i).cloned())
            .flatten()
    }

    fn reset_filter(&mut self) {
        self.filter = Filter::Disabled;
        self.visible_items = self.items.iter().cloned().collect();
        self.select_first_or_none();
    }

    fn update_filter(&mut self, event: &Event) where TablePage<T>: FilterItems<T>  {
        if let Filter::Input(filter_input) = &mut self.filter {
            filter_input.handle_event(event);
            let value = filter_input.value().to_string();
            self.visible_items = self
                .items
                .iter()
                .filter(|i| Self::matches(i.as_ref(), &value))
                .map(Rc::clone)
                .collect();
            self.select_first_or_none();
        }
    }

    fn show_filter(&mut self) {
        self.filter =  if let Filter::Value(filter_value) = &self.filter {
            Filter::Input(Input::new(filter_value.to_string()))
        }
        else {
            Filter::Input(Input::new("".to_string()))
        }

    }

    fn hide_filter(&mut self) {
        if let Filter::Input(filter_input) = &self.filter {
            self.filter = Filter::Value(filter_input.value().to_string());
        }
    }

    fn next_page(&self) {
        let mut table_state = self.table_state.borrow_mut();
        let new_selected = min(table_state.offset() + self.page_size.get(), self.visible_items.len() - 1);
        *table_state.offset_mut() = min(new_selected, self.visible_items.len().saturating_sub(self.page_size.get()
        ));
        table_state.select(Some(new_selected));
    }
    fn previous_page(&self) {
        let mut table_state = self.table_state.borrow_mut();
        let new_selected = max(table_state.offset().saturating_sub(self.page_size.get()), 0);
        *table_state.offset_mut() = new_selected;
        table_state.select(Some(new_selected));
    }

    fn instructions(&self) -> Title
    {
        let spans: Vec<Span> = self
            .actions
            .iter()
            .map(|c| {
                let span = Span::from(format!("  {}<{}>  ", c.name, c.shortcut));
                if (c.enabled)(self.selected_item().as_deref()) {
                    span
                } else {
                    span.fg(Color::DarkGray)
                }
            })
            .collect();

        Title::from(Line::from(spans))
    }

    fn rows(&self) -> Vec<Row> {
        self
            .visible_items
            .iter()
            .map(|i| {
                self.columns
                    .iter()
                    .map(|c| (c.get_value)(i.as_ref()))
                    .collect()
            })
            .collect()
    }

    fn table(&self) -> Table
    {
        let title = Title::from(self.title.clone().bold());

        let rows: Vec<Row> = self.rows();

        let block = Block::bordered()
            .title(title.alignment(Alignment::Center))
            .title(
                self.instructions()
                    .position(Position::Bottom)
                    .alignment(Alignment::Center),
            )
            .light_blue()
            .bg(Color::Black);
        let header_items: Vec<Span> = self
            .columns
            .iter()
            .map(|c| c.header.clone().bold().fg(Color::White))
            .collect();
        let header = Row::new(header_items);

        let width_constraints: Vec<Constraint> = self.columns.iter().map(|c| c.width).collect();
        Table::new(rows, width_constraints)
            .header(header)
            .highlight_style(Style::new().reversed())
            .block(block)
    }

    async fn go_back(&self) {
        self.message_tx.send(GoBack).await.unwrap()
    }

    pub async fn handle_event(&mut self, event: &Event) -> bool where TablePage<T>: FilterItems<T> {
        if self.filter.is_input() {
            match event {
                Event::Key(key_event) if key_event.code == KeyCode::Enter => {
                    self.hide_filter();
                    true
                },
                _ => {
                    // tui-input's handle_event doesn't indicate if it *actually* handled the event,
                    // but for our purposes, if the filter input is active, we assume it did.
                    self.update_filter(event);
                    true
                }
            };
            return true
        }

        if let Event::Key(key_event) = event {
            match key_event.code {
                KeyCode::Esc => {
                    if self.filter.is_active() {
                        self.reset_filter();
                    }
                    else {
                        self.go_back().await;
                    }
                    return true
                }
                KeyCode::Up => {
                    self.table_state.borrow_mut().select_previous();
                    return true;
                }
                KeyCode::Down => {
                    self.table_state.borrow_mut().select_next();
                    return true;
                },
                KeyCode::PageDown => {
                    self.next_page();
                    return true;
                },
                KeyCode::PageUp => {
                    self.previous_page();
                    return true;
                },
                KeyCode::Char('/') => {
                    self.show_filter();
                    return true;
                },
                _ => {} // Event not handled by basic navigation/filtering
            }
        }

        // If we reach here, the event was not handled by the table page itself.
        false
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {

        let layout_constraints = if self.filter.is_input() {
            [Constraint::Length(3), Constraint::Fill(1)]
        } else {
            [Constraint::Length(0), Constraint::Fill(1)]
        };

        let [search_area, table_area] = Layout::vertical(layout_constraints).areas(area);

        self.page_size.set(table_area.height as usize - 3);

        if let Filter::Input(search) = &self.filter {
            let block = Block::bordered().light_blue().on_black();
            let paragraph = Paragraph::new(format!("🔍{}", search.value()))
                .block(block)
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, search_area);
        }


        frame.render_stateful_widget(self.table(), table_area, &mut self.table_state.borrow_mut());
    }

}
