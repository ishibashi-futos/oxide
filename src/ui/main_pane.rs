use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::core::{ColorTheme, Entry};
use crate::ui::theme::to_color;

pub struct EntryListParams<'a> {
    pub entries: &'a [Entry],
    pub cursor: Option<usize>,
    pub title: &'a str,
    pub search_text: &'a str,
    pub theme: &'a ColorTheme,
    pub active: bool,
}

pub fn entry_list_view_height(area: Rect) -> usize {
    let inner_height = area.height.saturating_sub(2);
    inner_height as usize
}

pub fn render_entry_list(frame: &mut Frame<'_>, area: Rect, params: &EntryListParams<'_>) {
    let matches = search_matches(params.entries, params.search_text);
    let items: Vec<ListItem> = params
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut item = ListItem::new(display_name(entry));
            if matches.len() > 1 && matches.iter().skip(1).any(|&hit| hit == index) {
                item = item.style(secondary_match_style(params.theme));
            }
            item
        })
        .collect();

    let border_color = if params.active {
        to_color(params.theme.base)
    } else {
        to_color(params.theme.grayscale.low)
    };
    let text_style = if params.active {
        Style::default()
    } else {
        Style::default()
            .fg(to_color(params.theme.grayscale.high))
            .add_modifier(Modifier::DIM)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(params.title)
        .style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height > 0 {
        let list = List::new(items)
            .highlight_style(highlight_style(
                !params.search_text.is_empty(),
                params.theme,
            ))
            .highlight_symbol(highlight_symbol(params.search_text))
            .style(text_style);
        let mut state = ListState::default();
        state.select(params.cursor);
        frame.render_stateful_widget(list, inner, &mut state);
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
    fn entry_list_view_height_accounts_for_border() {
        let area = Rect::new(0, 0, 10, 6);

        let height = entry_list_view_height(area);

        assert_eq!(height, 4);
    }

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
                let params = EntryListParams {
                    entries: &entries,
                    cursor: Some(0),
                    title: "current",
                    search_text: "",
                    theme: &theme,
                    active: true,
                };
                render_entry_list(frame, area, &params)
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
                let params = EntryListParams {
                    entries: &entries,
                    cursor: Some(0),
                    title: "current",
                    search_text: "",
                    theme: &theme,
                    active: true,
                };
                render_entry_list(frame, area, &params)
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
                let params = EntryListParams {
                    entries: &entries,
                    cursor: Some(0),
                    title: "current",
                    search_text: "",
                    theme: &theme,
                    active: true,
                };
                render_entry_list(frame, area, &params)
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
                let params = EntryListParams {
                    entries: &entries,
                    cursor: Some(1),
                    title: "current",
                    search_text: "b",
                    theme: &theme,
                    active: true,
                };
                render_entry_list(frame, area, &params)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let style = find_cell_style(buffer, "bravo").expect("style not found");

        assert_eq!(style.fg, Some(to_color(theme.secondary)));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn render_search_footer_is_removed() {
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
                let params = EntryListParams {
                    entries: &entries,
                    cursor: Some(0),
                    title: "current",
                    search_text: "al",
                    theme: &theme,
                    active: true,
                };
                render_entry_list(frame, area, &params)
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 24, 6);
        assert!(!content.contains("search: al"));
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
