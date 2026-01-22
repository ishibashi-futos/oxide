use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::core::{ColorTheme, Entry};
use crate::ui::theme::to_color;

pub fn render_entry_list(
    frame: &mut Frame<'_>,
    area: Rect,
    entries: &[Entry],
    cursor: Option<usize>,
    title: &str,
    search_text: &str,
    theme: &ColorTheme,
    active: bool,
) {
    let matches = search_matches(entries, search_text);
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut item = ListItem::new(display_name(entry));
            if matches.len() > 1 && matches.iter().skip(1).any(|&hit| hit == index) {
                item = item.style(secondary_match_style(theme));
            }
            item
        })
        .collect();

    let border_color = if active {
        to_color(theme.base)
    } else {
        to_color(theme.grayscale.low)
    };
    let text_style = if active {
        Style::default()
    } else {
        Style::default()
            .fg(to_color(theme.grayscale.high))
            .add_modifier(Modifier::DIM)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (list_area, footer_area) = split_list_and_footer(inner, !search_text.is_empty());
    if list_area.height > 0 {
        let list = List::new(items)
            .highlight_style(highlight_style(!search_text.is_empty(), theme))
            .highlight_symbol(highlight_symbol(search_text))
            .style(text_style);
        let mut state = ListState::default();
        state.select(cursor);
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    if let Some(footer_area) = footer_area {
        let footer = build_search_footer(footer_area.width, search_text);
        let footer_widget = Paragraph::new(footer).style(search_footer_style());
        frame.render_widget(footer_widget, footer_area);
    }

    // ListState is handled above to keep footer aligned inside the border.
}

fn display_name(entry: &Entry) -> String {
    if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    }
}

fn highlight_style(search_active: bool, theme: &ColorTheme) -> Style {
    let style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(to_color(theme.primary));
    if search_active {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn secondary_match_style(theme: &ColorTheme) -> Style {
    Style::default()
        .fg(to_color(theme.secondary))
        .add_modifier(Modifier::DIM)
}

fn highlight_symbol(search_text: &str) -> &'static str {
    if search_text.is_empty() { "> " } else { "? " }
}

fn split_list_and_footer(area: Rect, show_footer: bool) -> (Rect, Option<Rect>) {
    if !show_footer || area.height == 0 {
        return (area, None);
    }
    let list_height = area.height.saturating_sub(1);
    let list_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: list_height,
    };
    let footer_area = Rect {
        x: area.x,
        y: area.y + list_height,
        width: area.width,
        height: 1,
    };
    (list_area, Some(footer_area))
}

fn build_search_footer(width: u16, search_text: &str) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    let prefix = "search: ";
    let mut result = String::new();
    let mut used = 0usize;
    for ch in prefix.chars() {
        if used >= width {
            return result;
        }
        result.push(ch);
        used += 1;
    }

    for ch in search_text.chars() {
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

fn search_footer_style() -> Style {
    Style::default().add_modifier(Modifier::UNDERLINED)
}

fn search_matches(entries: &[Entry], search_text: &str) -> Vec<usize> {
    if search_text.is_empty() {
        return Vec::new();
    }
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            if entry.name.starts_with(search_text) {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ColorThemeId, Entry};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::{Terminal, layout::Rect};

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
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(0), "current", "", &theme, true)
            })
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
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(0), "current", "", &theme, true)
            })
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
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(0), "current", "", &theme, true)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("docs/"));
    }

    #[test]
    fn highlight_style_changes_when_search_active() {
        let theme = ColorThemeId::GlacierCoast.theme();
        let normal = highlight_style(false, &theme);
        let searching = highlight_style(true, &theme);

        assert_ne!(normal, searching);
        assert_eq!(searching.fg, Some(to_color(theme.primary)));
        assert!(searching.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn highlight_symbol_changes_when_searching() {
        assert_eq!(highlight_symbol(""), "> ");
        assert_eq!(highlight_symbol("a"), "? ");
    }

    #[test]
    fn build_search_footer_fills_remaining_width() {
        let footer = build_search_footer(20, "alpha");
        assert_eq!(footer.chars().count(), 20);
        assert!(footer.contains("search: alpha"));
    }

    #[test]
    fn secondary_match_style_applies_to_non_selected_hits() {
        let backend = TestBackend::new(24, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries = vec![
            Entry {
                name: "alpha.txt".to_string(),
                is_dir: false,
            },
            Entry {
                name: "beta.txt".to_string(),
                is_dir: false,
            },
            Entry {
                name: "bravo.txt".to_string(),
                is_dir: false,
            },
        ];

        let area = Rect::new(0, 0, 24, 6);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(1), "current", "b", &theme, true)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "bravo").expect("style not found");

        assert_eq!(style.fg, Some(to_color(theme.secondary)));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }



    #[test]
    fn render_search_footer_in_current_panel() {
        let backend = TestBackend::new(24, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries = vec![Entry {
            name: "alpha.txt".to_string(),
            is_dir: false,
        }];

        let area = Rect::new(0, 0, 24, 6);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(0), "current", "al", &theme, true)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 24, 6);
        assert!(content.contains("search: al"));
    }

    #[test]
    fn search_footer_is_underlined() {
        let backend = TestBackend::new(24, 6);
        let mut terminal = Terminal::new(backend).unwrap();
        let entries = vec![Entry {
            name: "alpha.txt".to_string(),
            is_dir: false,
        }];

        let area = Rect::new(0, 0, 24, 6);
        let theme = ColorThemeId::GlacierCoast.theme();
        terminal
            .draw(|frame| {
                render_entry_list(frame, area, &entries, Some(0), "current", "al", &theme, true)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "search: al").expect("style not found");
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    }

    fn buffer_text(buffer: &Buffer, width: u16, height: u16) -> String {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| buffer[(x, y)].symbol().to_string()))
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
