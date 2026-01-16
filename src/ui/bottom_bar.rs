use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::app::App;
use crate::core::EntryMetadata;

pub fn render_bottom_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let bar = Paragraph::new(build_bottom_bar(app.selected_entry_metadata()));
    frame.render_widget(bar, area);
}

fn build_bottom_bar(metadata: Option<EntryMetadata>) -> String {
    match metadata {
        Some(metadata) => format_metadata(&metadata),
        None => String::new(),
    }
}

fn format_metadata(metadata: &EntryMetadata) -> String {
    format!(
        "size: {} | modified: {}",
        format_size(metadata.size),
        format_modified(metadata.modified)
    )
}

fn format_modified(modified: std::time::SystemTime) -> String {
    match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("{}s since epoch", duration.as_secs()),
        Err(_) => "unknown".to_string(),
    }
}

fn format_size(size: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const HALF_MB: u64 = MB / 2;
    const HALF_GB: u64 = GB / 2;

    if size >= HALF_GB {
        return format!("{:.1} GB", size as f64 / GB as f64);
    }
    if size >= HALF_MB {
        return format!("{:.1} MB", size as f64 / MB as f64);
    }
    format!("{size} B")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::{layout::Rect, Terminal};
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    use crate::core::Entry;

    #[test]
    fn format_metadata_uses_size_and_modified() {
        let metadata = EntryMetadata {
            size: 12,
            modified: UNIX_EPOCH + Duration::from_secs(0),
        };

        let formatted = format_metadata(&metadata);

        assert_eq!(
            formatted,
            "size: 12 B | modified: 0s since epoch"
        );
    }

    #[test]
    fn format_size_uses_bytes_below_half_mb() {
        let formatted = format_size(512 * 1024 - 1);

        assert_eq!(formatted, "524287 B");
    }

    #[test]
    fn format_size_uses_mb_at_half_mb_or_more() {
        let formatted = format_size(512 * 1024);

        assert_eq!(formatted, "0.5 MB");
    }

    #[test]
    fn format_size_uses_gb_at_half_gb_or_more() {
        let formatted = format_size(512 * 1024 * 1024);

        assert_eq!(formatted, "0.5 GB");
    }

    #[test]
    fn render_bottom_bar_shows_selected_entry_metadata() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let app = App::new(
            PathBuf::from(temp_dir.path()),
            vec![Entry {
                name: "note.txt".to_string(),
                is_dir: false,
            }],
            Vec::new(),
            Some(0),
            false,
        );

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        terminal
            .draw(|frame| render_bottom_bar(frame, area, &app))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("size: 5 B"));
        assert!(line.contains("modified:"));
    }

    fn buffer_line(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }
}
