use std::path::Path;
use std::sync::OnceLock;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntectColor, FontStyle, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

pub enum PreviewPaneState<'a> {
    Empty,
    Loading,
    Ready {
        lines: &'a [String],
        reason: Option<String>,
        truncated: bool,
        path: &'a Path,
    },
    Failed {
        reason: String,
    },
}

pub fn render_preview_pane(frame: &mut Frame<'_>, area: Rect, state: PreviewPaneState<'_>) {
    let block = Block::default().borders(Borders::ALL).title("preview");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }
    let text = Paragraph::new(build_preview_text(state));
    frame.render_widget(text, inner);
}

fn build_preview_text(state: PreviewPaneState<'_>) -> Text<'static> {
    match state {
        PreviewPaneState::Empty => Text::from("preview: empty"),
        PreviewPaneState::Loading => Text::from("preview: loading..."),
        PreviewPaneState::Ready {
            lines,
            reason,
            truncated,
            path,
        } => {
            let mut text = if reason.is_none() {
                highlight_preview_lines(lines, path).unwrap_or_else(|| plain_preview_lines(lines))
            } else {
                plain_preview_lines(lines)
            };
            if let Some(reason) = reason {
                append_plain_line(&mut text, reason);
            }
            if truncated {
                append_plain_line(&mut text, "…".to_string());
            }
            text
        }
        PreviewPaneState::Failed { reason } => Text::from(format!("preview failed: {reason}")),
    }
}

fn plain_preview_lines(lines: &[String]) -> Text<'static> {
    let mut text = Text::default();
    for line in lines {
        text.lines.push(Line::from(line.clone()));
    }
    text
}

fn append_plain_line(text: &mut Text<'static>, line: String) {
    text.lines.push(Line::from(line));
}

fn highlight_preview_lines(lines: &[String], path: &Path) -> Option<Text<'static>> {
    let syntax = syntax_for_path(path, lines)?;
    let mut highlighter = HighlightLines::new(syntax, highlight_theme());
    let mut highlighted = Vec::with_capacity(lines.len());
    for line in lines {
        let ranges = highlighter.highlight_line(line, syntax_set()).ok()?;
        let spans = ranges
            .into_iter()
            .map(|(style, text)| Span::styled(text.to_string(), syntect_to_style(style)))
            .collect::<Vec<_>>();
        highlighted.push(Line::from(spans));
    }
    Some(Text::from(highlighted))
}

fn syntax_for_path(path: &Path, lines: &[String]) -> Option<&'static SyntaxReference> {
    let syntax_set = syntax_set();
    if let Ok(Some(syntax)) = syntax_set.find_syntax_for_file(path) {
        return Some(syntax);
    }
    let first_line = lines.first().map(String::as_str).unwrap_or("");
    syntax_set.find_syntax_by_first_line(first_line)
}

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn highlight_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let theme_set = ThemeSet::load_defaults();
        theme_set
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| theme_set.themes.values().next().cloned().unwrap())
    })
}

fn syntect_to_style(style: syntect::highlighting::Style) -> Style {
    let mut out = Style::default();
    out = out.fg(to_ratatui_color(style.foreground));
    let mut modifiers = Modifier::empty();
    if style.font_style.contains(FontStyle::BOLD) {
        modifiers |= Modifier::BOLD;
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        modifiers |= Modifier::ITALIC;
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        modifiers |= Modifier::UNDERLINED;
    }
    out.add_modifier(modifiers)
}

fn to_ratatui_color(color: SyntectColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_preview_text_highlights_known_syntax() {
        let lines = vec!["fn main() {".to_string(), "}".to_string()];
        let text = build_preview_text(PreviewPaneState::Ready {
            lines: &lines,
            reason: None,
            truncated: false,
            path: Path::new("main.rs"),
        });

        assert!(text.lines[0].spans.len() > 1);
    }

    #[test]
    fn build_preview_text_falls_back_when_reason_is_present() {
        let lines = vec!["fn main() {".to_string()];
        let text = build_preview_text(PreviewPaneState::Ready {
            lines: &lines,
            reason: Some("non-utf8".to_string()),
            truncated: false,
            path: Path::new("main.rs"),
        });

        assert_eq!(text.lines[0].spans.len(), 1);
    }

    #[test]
    fn build_preview_text_falls_back_when_syntax_is_unknown() {
        let lines = vec!["plain text".to_string()];
        let text = build_preview_text(PreviewPaneState::Ready {
            lines: &lines,
            reason: None,
            truncated: false,
            path: Path::new("note.unknownext"),
        });

        assert_eq!(text.lines[0].spans.len(), 1);
    }
}
