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

use crate::{app::App, error::AppResult};

use event::{is_cursor_down_event, is_cursor_up_event, is_quit_event};
use layout::split_main;
use main_pane::render_directory_list;
use top_bar::render_top_bar;

pub fn run(mut app: App) -> AppResult<()> {
    let mut guard = TerminalGuard::new()?;

    loop {
        guard.terminal_mut().draw(|frame| draw(frame, &app))?;

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
        }
    }

    Ok(())
}

fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let (top, main) = split_main(area);
    render_top_bar(frame, top, app);
    render_directory_list(frame, main, app);
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
