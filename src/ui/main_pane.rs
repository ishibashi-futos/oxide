use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::core::Entry;

pub fn render_entry_list(
    frame: &mut Frame<'_>,
    area: Rect,
    entries: &[Entry],
    cursor: Option<usize>,
    title: &str,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| ListItem::new(display_name(entry)))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(cursor);
    frame.render_stateful_widget(list, area, &mut state);
}

fn display_name(entry: &Entry) -> String {
    if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::{layout::Rect, Terminal};
    use crate::core::Entry;

    #[test]
    fn render_directory_list_shows_entries() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries = vec![
            Entry {
                name: "a.txt".to_string(),
                is_dir: false,
            },
            Entry {
                name: "b.txt".to_string(),
                is_dir: false,
            },
        ];

        let area = Rect::new(0, 0, 20, 5);
        terminal
            .draw(|frame| render_entry_list(frame, area, &entries, Some(0), "current"))
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
        let entries = vec![
            Entry {
                name: "a.txt".to_string(),
                is_dir: false,
            },
            Entry {
                name: "b.txt".to_string(),
                is_dir: false,
            },
        ];

        let area = Rect::new(0, 0, 20, 5);
        terminal
            .draw(|frame| render_entry_list(frame, area, &entries, Some(0), "current"))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("> "));
    }

    #[test]
    fn render_directory_list_adds_trailing_slash_for_directories() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries = vec![Entry {
            name: "docs".to_string(),
            is_dir: true,
        }];

        let area = Rect::new(0, 0, 20, 5);
        terminal
            .draw(|frame| render_entry_list(frame, area, &entries, Some(0), "current"))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("docs/"));
    }

    fn buffer_text(buffer: &Buffer, width: u16, height: u16) -> String {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| buffer[(x, y)].symbol().to_string()))
            .collect()
    }
}
