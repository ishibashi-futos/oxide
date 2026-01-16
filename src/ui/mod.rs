mod event;
mod layout;
mod main_pane;
mod top_bar;

use std::io::{self, Stdout};

use crossterm::{
    cursor::{Hide, Show},
    event::{self as crossterm_event, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};

use crate::{
    app::{App, EntryOpener},
    error::AppResult,
};

use event::{
    is_cursor_down_event, is_cursor_up_event, is_enter_dir_event, is_enter_event,
    is_parent_event, is_quit_event,
};
use layout::{split_main, split_panes};
use main_pane::render_entry_list;
use top_bar::render_top_bar;

pub fn run(mut app: App, opener: &dyn EntryOpener) -> AppResult<()> {
    let mut guard = TerminalGuard::new()?;

    loop {
        guard
            .terminal_mut()
            .draw(|frame| draw(frame, &app))?;

        if let Event::Key(key) = crossterm_event::read()? {
            if is_quit_event(key) {
                break;
            }
            if is_cursor_up_event(key) {
                app.move_cursor_up();
            }
            if is_cursor_down_event(key) {
                app.move_cursor_down();
            }
            if is_enter_event(key) {
                app.open_selected(opener)?;
            }
            if is_enter_dir_event(key) {
                app.enter_selected_dir()?;
            }
            if is_parent_event(key) {
                app.move_to_parent()?;
            }
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let (top, main) = split_main(area);
    render_top_bar(frame, top, app);
    let (left, right) = split_panes(main);
    render_entry_list(frame, left, &app.parent_entries, None, "parent");
    render_entry_list(frame, right, &app.entries, app.cursor, "current");
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
