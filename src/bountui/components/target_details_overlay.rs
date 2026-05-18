use crate::boundary::Target;
use crossterm::event::{Event, KeyCode};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::cell::Cell;

pub struct TargetDetailsOverlay {
    target: Target,
    /// Scroll offset for long content (0 = top), uses Cell for &self mutability
    scroll_offset: Cell<u16>,
}

impl TargetDetailsOverlay {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            scroll_offset: Cell::new(0),
        }
    }

    /// Build the full set of detail lines from the target
    fn detail_lines(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();

        // Basic fields
        self.add_kv(&mut lines, "Name", self.target.name.clone());
        self.add_kv(&mut lines, "ID", self.target.id.clone());
        self.add_kv(&mut lines, "Type", self.target.type_name.clone());
        self.add_kv(&mut lines, "Scope ID", self.target.scope_id.clone());

        if !self.target.description.is_empty() {
            self.add_kv(&mut lines, "Description", self.target.description.clone());
        }

        // Default client port
        if let Some(port) = self.target.default_client_port() {
            self.add_kv(&mut lines, "Default Port", port.to_string());
        }

        // Connection capability
        self.add_kv(&mut lines, "Can Connect", self.target.can_connect().to_string());

        // Authorized actions
        if !self.target.authorized_actions.is_empty() {
            self.add_header(&mut lines, "Authorized Actions");
            for action in &self.target.authorized_actions {
                lines.push(Line::from(format!("  \u{2022} {}", action)));
            }
        }

        // Authorized collection actions
        if !self.target.authorized_collection_actions.is_empty() {
            self.add_header(&mut lines, "Collection Actions");
            self.format_collection_actions(&mut lines);
        }

        // Footer hint
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Press ESC or 'd' to close",
            Style::default().fg(Color::DarkGray),
        ));

        lines
    }

    fn add_kv(&self, lines: &mut Vec<Line<'static>>, key: &str, value: String) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", key),
                Style::default().fg(Color::LightCyan).bold(),
            ),
            Span::raw(value),
        ]));
    }

    fn add_header(&self, lines: &mut Vec<Line<'static>>, title: &str) {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("\u{2500}\u{2500} {} \u{2500}\u{2500}", title),
            Style::default().fg(Color::LightBlue).bold(),
        ));
    }

    fn format_collection_actions(&self, lines: &mut Vec<Line<'static>>) {
        let mut entries: Vec<(&String, &Vec<String>)> =
            self.target.authorized_collection_actions.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        for (collection, actions) in entries {
            let action_list = actions.join(", ");
            lines.push(Line::from(format!(
                "  \u{2022} {}: [{}]",
                collection, action_list
            )));
        }
    }

    pub fn view(&self, frame: &mut Frame) {
        let area = frame.area();

        // Center the overlay: 70% width, up to 80% height
        let overlay_width = Constraint::Percentage(70);
        let overlay_height = Constraint::Percentage(80);

        let [overlay_area] = Layout::vertical([overlay_height])
            .flex(Flex::Center)
            .areas(area);
        let [overlay_area] = Layout::horizontal([overlay_width])
            .flex(Flex::Center)
            .areas(overlay_area);

        frame.render_widget(Clear, overlay_area);

        let title = format!("Target: {}", self.target.name);
        let block = Block::default()
            .title(title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .light_blue()
            .on_black();

        let inner_area = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        let all_lines = self.detail_lines();
        let total_lines = all_lines.len() as u16;
        let content_height = inner_area.height;

        // Clamp scroll
        let max_scroll = total_lines.saturating_sub(content_height);
        let mut scroll = self.scroll_offset.get();
        scroll = scroll.min(max_scroll);
        self.scroll_offset.set(scroll);

        // Slice the visible portion
        let start = scroll as usize;
        let end = (start + content_height as usize).min(all_lines.len());
        let visible: Vec<Line> = all_lines[start..end].to_vec();

        let paragraph = Paragraph::new(Text::from(visible)).alignment(Alignment::Left);

        frame.render_widget(paragraph, inner_area);

        // Scroll indicator
        if max_scroll > 0 {
            let pct = if max_scroll == 0 {
                100
            } else {
                (scroll as f64 / max_scroll as f64 * 100.0) as u16
            };

            let indicator = format!(" {}/{} ({}%) ", scroll + 1, max_scroll + 1, pct);
            let indicator_w = indicator.len() as u16;

            // Position at bottom-right of the overlay
            let indicator_rect = Rect {
                x: overlay_area.right().saturating_sub(indicator_w + 2),
                y: overlay_area.bottom().saturating_sub(1),
                width: indicator_w,
                height: 1,
            };
            let indicator_style = Style::default().fg(Color::DarkGray).bg(Color::Black);
            frame.render_widget(
                Paragraph::new(indicator).style(indicator_style),
                indicator_rect,
            );
        }
    }

    pub fn handle_event(&self, event: &Event) -> bool {
        // Returns true if overlay should close
        if let Event::Key(key_event) = event {
            let scroll = self.scroll_offset.get();
            match key_event.code {
                KeyCode::Esc => {
                    return true; // Close
                }
                KeyCode::Char('d') => {
                    return true; // Also close on 'd' (toggle behavior)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if scroll > 0 {
                        self.scroll_offset.set(scroll.saturating_sub(1));
                    }
                    return false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // Need to recompute max_scroll; we don't have it stored.
                    // Use a generous upper bound — detail_lines length.
                    let total_lines = self.detail_lines().len() as u16;
                    // content_height unknown here (depends on frame area at view time).
                    // Use a reasonable estimate; the real clamp happens in view().
                    if scroll + 1 < total_lines {
                        self.scroll_offset.set(scroll + 1);
                    }
                    return false;
                }
                KeyCode::PageUp => {
                    self.scroll_offset
                        .set(scroll.saturating_sub(10));
                    return false;
                }
                KeyCode::PageDown => {
                    self.scroll_offset.set(scroll.saturating_add(10));
                    return false;
                }
                KeyCode::Home => {
                    self.scroll_offset.set(0);
                    return false;
                }
                KeyCode::End => {
                    self.scroll_offset.set(u16::MAX);
                    // view() will clamp this
                    return false;
                }
                _ => {
                    // Consume all other keys
                    return false;
                }
            }
        }
        false
    }
}
