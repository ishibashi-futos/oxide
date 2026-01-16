use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::app::App;

pub fn render_top_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let path = app.current_dir.to_string_lossy();
    let active = app
        .selected_entry()
        .map(|entry| entry.name.as_str())
        .unwrap_or("");
    let bar = Paragraph::new(format!("{} | {}", path, active));
    frame.render_widget(bar, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::{layout::Rect, Terminal};
    use std::path::PathBuf;
    use crate::core::Entry;

    #[test]
    fn render_top_bar_shows_current_path_and_active_item() {
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(
            PathBuf::from("/tmp"),
            vec![Entry {
                name: "a.txt".to_string(),
                is_dir: false,
            }],
            Vec::new(),
            Some(0),
        );

        let area = Rect::new(0, 0, 30, 1);
        terminal
            .draw(|frame| render_top_bar(frame, area, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 30);

        assert!(line.contains("/tmp"));
        assert!(line.contains("a.txt"));
    }

    #[test]
    fn render_top_bar_shows_active_item_from_cursor() {
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(
            PathBuf::from("/tmp"),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
        );

        let area = Rect::new(0, 0, 30, 1);
        terminal
            .draw(|frame| render_top_bar(frame, area, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 30);

        assert!(line.contains("b.txt"));
    }

    fn buffer_line(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }
}
