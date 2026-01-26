use chrono::{DateTime, Local};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{SlashCandidates, SlashFeedback};
use crate::core::ColorTheme;
use crate::core::user_notice::{UserNotice, UserNoticeLevel};
use crate::core::{EntryMetadata, MetadataStatus};
use crate::tabs::TabSummary;
use crate::ui::theme::to_color;

pub fn render_bottom_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    metadata: Option<&str>,
    metadata_status: Option<MetadataStatus>,
    git: Option<&str>,
    notice: Option<&UserNotice>,
    feedback: Option<&SlashFeedback>,
    theme: &ColorTheme,
) {
    let bar = Paragraph::new(build_bottom_bar(
        metadata,
        metadata_status,
        git,
        notice,
        feedback,
        area.width,
        theme,
    ));
    frame.render_widget(
        bar.style(Style::default().bg(to_color(theme.grayscale.low))),
        area,
    );
}

pub fn render_slash_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    input: &str,
    candidates: &SlashCandidates,
    hint: Option<&str>,
    theme: &ColorTheme,
) {
    let bar = Paragraph::new(build_slash_bar(input, candidates, hint, area.width))
        .style(slash_bar_style(theme));
    frame.render_widget(bar, area);
}

pub fn render_search_bar(frame: &mut Frame<'_>, area: Rect, input: &str, theme: &ColorTheme) {
    let suffix_style = Style::default()
        .fg(to_color(theme.grayscale.high))
        .add_modifier(Modifier::DIM);
    let bar = Paragraph::new(build_search_bar_line(
        input,
        " - incremental search",
        suffix_style,
        area.width,
    ))
    .style(slash_bar_style(theme));
    frame.render_widget(bar, area);
}

fn build_bottom_bar(
    metadata: Option<&str>,
    metadata_status: Option<MetadataStatus>,
    git: Option<&str>,
    notice: Option<&UserNotice>,
    feedback: Option<&SlashFeedback>,
    width: u16,
    theme: &ColorTheme,
) -> Line<'static> {
    let default_style = Style::default().fg(to_color(theme.grayscale.high));
    let mut left_spans = Vec::new();
    if let Some(notice) = notice {
        let text = format_notice_text(notice);
        if !text.is_empty() {
            let style = notice_style(notice.level, theme);
            left_spans.push(Span::styled(text, style));
        }
    } else if let Some(feedback) = feedback {
        let (text, style) = if let Some(tabs) = feedback.tabs.as_deref() {
            (
                format_tabs(tabs),
                Style::default().fg(to_color(theme.secondary)),
            )
        } else {
            let style = match feedback.status {
                crate::app::FeedbackStatus::Success => {
                    Style::default().fg(to_color(theme.semantic.success))
                }
                crate::app::FeedbackStatus::Error => {
                    Style::default().fg(to_color(theme.semantic.error))
                }
                crate::app::FeedbackStatus::Warn => {
                    Style::default().fg(to_color(theme.semantic.warn))
                }
            };
            (feedback.text.clone(), style)
        };
        if !text.is_empty() {
            left_spans.push(Span::styled(text, style));
        }
    }
    let (metadata_text, metadata_style) = metadata_parts(metadata, metadata_status, theme);
    if !metadata_text.is_empty() {
        if !left_spans.is_empty() {
            left_spans.push(Span::styled(" | ", default_style));
        }
        left_spans.push(Span::styled(metadata_text, metadata_style));
    }

    let git_line = git
        .map(|value| value.to_string())
        .unwrap_or_else(placeholder_git);
    line_with_right_spans(
        left_spans,
        vec![Span::styled(git_line, default_style)],
        width,
    )
}

fn format_notice_text(notice: &UserNotice) -> String {
    let text = notice.text.trim();
    let source = notice.source.trim();
    if text.is_empty() && source.is_empty() {
        return String::new();
    }
    if source.is_empty() {
        return format!("{} {}", notice.level.icon(), text);
    }
    if text.is_empty() {
        return format!("{} {}", notice.level.icon(), source);
    }
    format!("{} {}: {}", notice.level.icon(), source, text)
}

fn notice_style(level: UserNoticeLevel, theme: &ColorTheme) -> Style {
    let color = match level {
        UserNoticeLevel::Success => theme.semantic.success,
        UserNoticeLevel::Info => theme.semantic.info,
        UserNoticeLevel::Warn => theme.semantic.warn,
        UserNoticeLevel::Error => theme.semantic.error,
    };
    Style::default().fg(to_color(color))
}

fn build_slash_bar(
    input: &str,
    candidates: &SlashCandidates,
    hint: Option<&str>,
    width: u16,
) -> String {
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
    } else if !candidates.items.is_empty() {
        full.push(' ');
        full.push(' ');
        full.push_str(
            &candidates
                .items
                .iter()
                .map(|candidate| match &candidate.description {
                    Some(description) => format!("{}({})", candidate.text, description),
                    None => candidate.text.clone(),
                })
                .collect::<Vec<String>>()
                .join(" "),
        );
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

fn build_search_bar_line(
    input: &str,
    suffix: &str,
    suffix_style: Style,
    width: u16,
) -> Line<'static> {
    let width = width as usize;
    if width == 0 {
        return Line::default();
    }
    let mut used = 0usize;
    let mut spans = Vec::new();

    let mut input_text = String::new();
    for ch in input.chars() {
        if used >= width {
            break;
        }
        input_text.push(ch);
        used += 1;
    }
    if !input_text.is_empty() {
        spans.push(Span::raw(input_text));
    }

    if used < width {
        let mut suffix_text = String::new();
        for ch in suffix.chars() {
            if used >= width {
                break;
            }
            suffix_text.push(ch);
            used += 1;
        }
        if !suffix_text.is_empty() {
            spans.push(Span::styled(suffix_text, suffix_style));
        }
    }

    if used < width {
        spans.push(Span::raw(" ".repeat(width - used)));
    }
    Line::from(spans)
}

fn line_with_right_spans(
    mut left: Vec<Span<'static>>,
    mut right: Vec<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let left_len = spans_len(&left);
    let right_len = spans_len(&right);
    let total_width = width as usize;
    if total_width <= left_len + right_len + 1 {
        left.push(Span::raw(" "));
        left.append(&mut right);
        return Line::from(left);
    }
    let spaces = total_width - left_len - right_len;
    left.push(Span::raw(" ".repeat(spaces)));
    left.append(&mut right);
    Line::from(left)
}

fn spans_len(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn slash_bar_style(theme: &ColorTheme) -> Style {
    Style::default()
        .bg(to_color(theme.grayscale.low))
        .fg(to_color(theme.grayscale.high))
}

fn format_tabs(tabs: &[TabSummary]) -> String {
    if tabs.is_empty() {
        return "tabs:".to_string();
    }
    let entries = tabs
        .iter()
        .map(|summary| {
            let marker = if summary.active { "*" } else { "" };
            format!("{}{}:{}", summary.number, marker, summary.path.display())
        })
        .collect::<Vec<String>>();
    format!("tabs: {}", entries.join(" "))
}

pub fn format_metadata(metadata: &EntryMetadata) -> String {
    format!(
        "size: {} | modified: {}",
        format_size(metadata.size),
        format_modified(metadata.modified)
    )
}

fn metadata_parts(
    metadata: Option<&str>,
    metadata_status: Option<MetadataStatus>,
    theme: &ColorTheme,
) -> (String, Style) {
    match metadata_status {
        Some(MetadataStatus::Loading) => (
            "metadata: loading".to_string(),
            Style::default().fg(to_color(theme.semantic.info)),
        ),
        Some(MetadataStatus::Error) => (
            "metadata: error".to_string(),
            Style::default().fg(to_color(theme.semantic.error)),
        ),
        None => (
            metadata.map(|value| value.to_string()).unwrap_or_default(),
            Style::default().fg(to_color(theme.grayscale.high)),
        ),
    }
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
    use crate::app::{SlashCandidate, SlashCandidates};
    use crate::core::ColorThemeId;
    use crate::ui::theme::to_color;
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
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(
                    frame,
                    area,
                    Some(&metadata_line),
                    None,
                    Some("git: main"),
                    None,
                    None,
                    &theme,
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 120);

        assert!(line.contains("size: 5 B"));
        assert!(line.contains("modified:"));
    }

    #[test]
    fn render_bottom_bar_hides_metadata_when_missing() {
        let backend = TestBackend::new(30, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 30, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| render_bottom_bar(frame, area, None, None, None, None, None, &theme))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 30);

        assert!(!line.contains("size: - | modified: -"));
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
            tabs: None,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(1),
        };

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(
                    frame,
                    area,
                    Some(&metadata_line),
                    None,
                    Some("git: main"),
                    None,
                    Some(&feedback),
                    &theme,
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("preview: on"));
        assert!(line.contains("size: 7 B"));
    }

    #[test]
    fn render_bottom_bar_shows_user_notice() {
        let notice = UserNotice::new(UserNoticeLevel::Info, "exit=0", "shell");

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(frame, area, None, None, None, Some(&notice), None, &theme)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("shell: exit=0"));
    }

    #[test]
    fn user_notice_uses_semantic_color() {
        let notice = UserNotice::new(UserNoticeLevel::Warn, "save failed", "session");

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(frame, area, None, None, None, Some(&notice), None, &theme)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "session").expect("style not found");

        assert_eq!(style.fg, Some(to_color(theme.semantic.warn)));
    }

    #[test]
    fn render_bottom_bar_formats_tab_feedback() {
        let feedback = SlashFeedback {
            text: String::new(),
            status: crate::app::FeedbackStatus::Success,
            tabs: Some(vec![
                TabSummary {
                    number: 1,
                    path: std::path::PathBuf::from("/one"),
                    active: false,
                },
                TabSummary {
                    number: 2,
                    path: std::path::PathBuf::from("/two"),
                    active: true,
                },
            ]),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(1),
        };

        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 60, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(frame, area, None, None, None, None, Some(&feedback), &theme)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("tabs: 1:/one 2*:/two"));
    }

    #[test]
    fn tab_feedback_uses_secondary_color() {
        let feedback = SlashFeedback {
            text: String::new(),
            status: crate::app::FeedbackStatus::Success,
            tabs: Some(vec![TabSummary {
                number: 1,
                path: std::path::PathBuf::from("/one"),
                active: true,
            }]),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(1),
        };

        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 40, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(frame, area, None, None, None, None, Some(&feedback), &theme)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "tabs:").expect("style not found");
        assert_eq!(style.fg, Some(to_color(theme.secondary)));
    }

    #[test]
    fn render_slash_bar_shows_input_text() {
        let backend = TestBackend::new(20, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 20, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_slash_bar(
                    frame,
                    area,
                    "/preview",
                    &SlashCandidates::default(),
                    None,
                    &theme,
                )
            })
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
        let candidates = SlashCandidates {
            items: vec![
                SlashCandidate {
                    text: "/preview".to_string(),
                    description: None,
                },
                SlashCandidate {
                    text: "/paste".to_string(),
                    description: None,
                },
            ],
        };
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| render_slash_bar(frame, area, "/p", &candidates, None, &theme))
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
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_slash_bar(
                    frame,
                    area,
                    "/preview",
                    &SlashCandidates::default(),
                    Some("toggle preview | options: show, hide"),
                    &theme,
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 60);

        assert!(line.contains("toggle preview"));
        assert!(line.contains("options: show, hide"));
    }

    #[test]
    fn render_search_bar_shows_input_and_suffix() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 40, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| render_search_bar(frame, area, "alpha", &theme))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 40);

        assert!(line.contains("alpha"));
        assert!(line.contains("incremental search"));
    }

    #[test]
    fn render_search_bar_uses_dim_style_for_suffix() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 40, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| render_search_bar(frame, area, "alpha", &theme))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "incremental").expect("style not found");

        assert!(style.add_modifier.contains(Modifier::DIM));
        assert_eq!(style.fg, Some(to_color(theme.grayscale.high)));
    }

    #[test]
    fn render_bottom_bar_shows_metadata_loading() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 40, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(
                    frame,
                    area,
                    None,
                    Some(MetadataStatus::Loading),
                    None,
                    None,
                    None,
                    &theme,
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 40);

        assert!(line.contains("metadata: loading"));
    }

    #[test]
    fn render_bottom_bar_shows_metadata_error() {
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let area = Rect::new(0, 0, 40, 1);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_bottom_bar(
                    frame,
                    area,
                    None,
                    Some(MetadataStatus::Error),
                    None,
                    None,
                    None,
                    &theme,
                )
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let line = buffer_line(buffer, 0, 40);

        assert!(line.contains("metadata: error"));
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

    fn find_cell_style(buffer: &Buffer, needle: &str) -> Option<Style> {
        let width = buffer.area.width;
        let height = buffer.area.height;
        let needle_chars: Vec<char> = needle.chars().collect();
        for y in 0..height {
            for x in 0..width {
                if buffer[(x, y)].symbol().chars().next()? != needle_chars[0] {
                    continue;
                }
                let mut matched = true;
                for (offset, ch) in needle_chars.iter().enumerate() {
                    let xi = x + offset as u16;
                    if xi >= width {
                        matched = false;
                        break;
                    }
                    let cell_ch = buffer[(xi, y)].symbol().chars().next().unwrap_or('\0');
                    if cell_ch != *ch {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Some(buffer[(x, y)].style());
                }
            }
        }
        None
    }
}
