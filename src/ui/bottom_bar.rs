use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::app::SlashFeedback;
use crate::core::EntryMetadata;

pub fn render_bottom_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    metadata: Option<&str>,
    git: Option<&str>,
    feedback: Option<&SlashFeedback>,
) {
    let bar = Paragraph::new(build_bottom_bar(metadata, git, feedback, area.width));
    frame.render_widget(bar, area);
}

pub fn render_slash_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    input: &str,
    candidates: &[String],
    hint: Option<&str>,
) {
    let bar = Paragraph::new(build_slash_bar(input, candidates, hint, area.width))
        .style(slash_bar_style());
    frame.render_widget(bar, area);
}

fn build_bottom_bar(
    metadata: Option<&str>,
    git: Option<&str>,
    feedback: Option<&SlashFeedback>,
    width: u16,
) -> String {
    let metadata_line = metadata
        .map(|value| value.to_string())
        .unwrap_or_else(placeholder_metadata);
    let left = if let Some(feedback) = feedback {
        if feedback.text.is_empty() {
            metadata_line
        } else {
            format!("{} | {}", feedback.text, metadata_line)
        }
    } else {
        metadata_line
    };
    let git_line = git
        .map(|value| value.to_string())
        .unwrap_or_else(placeholder_git);
    line_with_right(left, git_line, width)
}

fn build_slash_bar(input: &str, candidates: &[String], hint: Option<&str>, width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    let mut full = input.to_string();
    if let Some(hint) = hint {
        if !hint.is_empty() {
            full.push(' ');
            full.push(' ');
            full.push_str(hint);
        }
    } else if !candidates.is_empty() {
        full.push(' ');
        full.push(' ');
        full.push_str(&candidates.join(" "));
    }
    let mut result = String::new();
    let mut used = 0usize;
    for ch in full.chars() {
        if used >= width {
            return result;
        }
        result.push(ch);
        used += 1;
    }
    while used < width {
        result.push(' ');
        used += 1;
    }
    result
}

fn line_with_right(left: String, right: String, width: u16) -> String {
    let left_len = left.chars().count();
    let right_len = right.chars().count();
    let total_width = width as usize;
    if total_width <= left_len + right_len + 1 {
        return format!("{left} {right}");
    }
    let spaces = total_width - left_len - right_len;
    format!("{left}{}{right}", " ".repeat(spaces))
}

fn slash_bar_style() -> Style {
    Style::default().bg(Color::DarkGray).fg(Color::White)
}

pub fn format_metadata(metadata: &EntryMetadata) -> String {
    format!(
        "size: {} | modified: {}",
        format_size(metadata.size),
        format_modified(metadata.modified)
    )
}

fn placeholder_metadata() -> String {
    "size: - | modified: -".to_string()
}

fn placeholder_git() -> String {
    "git: -".to_string()
}

fn format_modified(modified: std::time::SystemTime) -> String {
    let datetime: DateTime<Local> = modified.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
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
    use ratatui::{Terminal, layout::Rect};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn format_metadata_uses_size_and_modified() {
        let metadata = EntryMetadata {
            size: 12,
            modified: UNIX_EPOCH + Duration::from_secs(0),
        };

        let formatted = format_metadata(&metadata);

        assert!(formatted.starts_with("size: 12 B | modified: "));
        assert_datetime_format(formatted.trim_start_matches("size: 12 B | modified: "));
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
        let metadata = EntryMetadata {
            size: 5,
            modified: UNIX_EPOCH + Duration::from_secs(0),
        };
        let metadata_line = format_metadata(&metadata);

        let backend = TestBackend::new(120, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 120, 1);
        terminal
            .draw(|frame| {
                render_bottom_bar(frame, area, Some(&metadata_line), Some("git: main"), None)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 120);

        assert!(line.contains("size: 5 B"));
        assert!(line.contains("modified:"));
    }

    #[test]
    fn render_bottom_bar_shows_placeholder_when_missing() {
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 30, 1);
        terminal
            .draw(|frame| render_bottom_bar(frame, area, None, None, None))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 30);

        assert!(line.contains("size: - | modified: -"));
        assert!(line.contains("git: -"));
    }

    #[test]
    fn render_bottom_bar_keeps_metadata_when_feedback_present() {
        let metadata = EntryMetadata {
            size: 7,
            modified: UNIX_EPOCH + Duration::from_secs(0),
        };
        let metadata_line = format_metadata(&metadata);
        let feedback = SlashFeedback {
            text: "preview: on".to_string(),
            status: crate::app::FeedbackStatus::Success,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(1),
        };

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        terminal
            .draw(|frame| {
                render_bottom_bar(
                    frame,
                    area,
                    Some(&metadata_line),
                    Some("git: main"),
                    Some(&feedback),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("preview: on"));
        assert!(line.contains("size: 7 B"));
    }

    #[test]
    fn render_slash_bar_shows_input_text() {
        let backend = TestBackend::new(20, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 20, 1);
        terminal
            .draw(|frame| render_slash_bar(frame, area, "/preview", &[], None))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 20);

        assert!(line.contains("/preview"));
    }

    #[test]
    fn render_slash_bar_shows_candidates() {
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 30, 1);
        let candidates = vec!["/preview".to_string(), "/paste".to_string()];
        terminal
            .draw(|frame| render_slash_bar(frame, area, "/p", &candidates, None))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 30);

        assert!(line.contains("/preview"));
        assert!(line.contains("/paste"));
    }

    #[test]
    fn render_slash_bar_shows_hint_when_present() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        terminal
            .draw(|frame| {
                render_slash_bar(
                    frame,
                    area,
                    "/preview",
                    &[],
                    Some("toggle preview | options: show, hide"),
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("toggle preview"));
        assert!(line.contains("options: show, hide"));
    }

    fn assert_datetime_format(value: &str) {
        let chars: Vec<char> = value.chars().collect();
        assert_eq!(chars.len(), 19);
        assert_eq!(chars[4], '-');
        assert_eq!(chars[7], '-');
        assert_eq!(chars[10], ' ');
        assert_eq!(chars[13], ':');
        assert_eq!(chars[16], ':');
        for (index, ch) in chars.iter().enumerate() {
            if matches!(index, 4 | 7 | 10 | 13 | 16) {
                continue;
            }
            assert!(ch.is_ascii_digit());
        }
    }

    fn buffer_line(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }
}
