use std::io::{self, Stdout};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

use crate::{app::App, error::AppResult};

pub fn run(mut app: App) -> AppResult<()> {
    let mut guard = TerminalGuard::new()?;

    loop {
        guard
            .terminal_mut()
            .draw(|frame| render_main_pane(frame, &app))?;

        if let Event::Key(key) = event::read()? {
            if is_quit_event(key) {
                break;
            }
            if is_cursor_up_event(key) {
                app.move_cursor_up();
            }
            if is_cursor_down_event(key) {
                app.move_cursor_down();
            }
        }
    }

    Ok(())
}

fn is_quit_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        && key.code == KeyCode::Char('q')
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_cursor_up_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Up
}

fn is_cursor_down_event(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press && key.code == KeyCode::Down
}

pub fn render_main_pane(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(area);
    render_top_bar(frame, chunks[0], app);
    render_directory_list(frame, chunks[1], app);
}

fn render_directory_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
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

fn render_top_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let path = app.current_dir.to_string_lossy();
    let active = app
        .cursor
        .and_then(|index| app.entries.get(index))
        .map(String::as_str)
        .unwrap_or("");
    let bar = Paragraph::new(format!("{} | {}", path, active));
    frame.render_widget(bar, area);
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> AppResult<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = restore_terminal();
                Err(error.into())
            }
        }
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

fn restore_terminal() -> std::io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, Show)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;
    use std::path::PathBuf;

    #[test]
    fn render_main_pane_shows_entries() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(PathBuf::from("."), vec!["a.txt".into(), "b.txt".into()], Some(0));

        terminal.draw(|frame| render_main_pane(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("a.txt"));
        assert!(content.contains("b.txt"));
    }

    #[test]
    fn render_main_pane_shows_current_path_in_top_bar() {
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(PathBuf::from("/tmp"), vec!["a.txt".into()], Some(0));

        terminal.draw(|frame| render_main_pane(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let top_line = buffer_line(buffer, 0, 30);

        assert!(top_line.contains("/tmp"));
        assert!(top_line.contains("a.txt"));
    }

    #[test]
    fn render_main_pane_shows_active_item_from_cursor() {
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(
            PathBuf::from("/tmp"),
            vec!["a.txt".into(), "b.txt".into()],
            Some(1),
        );

        terminal.draw(|frame| render_main_pane(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let top_line = buffer_line(buffer, 0, 30);

        assert!(top_line.contains("b.txt"));
    }

    #[test]
    fn render_main_pane_highlights_selected_item() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(PathBuf::from("."), vec!["a.txt".into(), "b.txt".into()], Some(0));

        terminal.draw(|frame| render_main_pane(frame, &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_text(buffer, 20, 5);

        assert!(content.contains("> "));
    }

    fn buffer_line(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }

    fn buffer_text(buffer: &Buffer, width: u16, height: u16) -> String {
        (0..height)
            .map(|y| buffer_line(buffer, y, width))
            .collect()
    }
}
