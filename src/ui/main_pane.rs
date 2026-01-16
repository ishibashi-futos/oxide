use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::App;

pub fn render_directory_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .entries
        .iter()
        .map(|entry| ListItem::new(entry.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("ox"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(app.cursor);
    frame.render_stateful_widget(list, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::{layout::Rect, Terminal};
    use std::path::PathBuf;

    #[test]
    fn render_directory_list_shows_entries() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(PathBuf::from("."), vec!["a.txt".into(), "b.txt".into()], Some(0));

        let area = Rect::new(0, 0, 20, 5);
        terminal
            .draw(|frame| render_directory_list(frame, area, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("a.txt"));
        assert!(content.contains("b.txt"));
    }

    #[test]
    fn render_directory_list_highlights_selected_item() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(PathBuf::from("."), vec!["a.txt".into(), "b.txt".into()], Some(0));

        let area = Rect::new(0, 0, 20, 5);
        terminal
            .draw(|frame| render_directory_list(frame, area, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("> "));
    }

    fn buffer_text(buffer: &Buffer, width: u16, height: u16) -> String {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect()
    }
}
