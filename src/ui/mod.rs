mod bottom_bar;
mod event;
mod layout;
mod main_pane;
mod metadata_worker;
mod top_bar;

use std::io::{self, Stdout};

use crossterm::{
    cursor::{Hide, Show},
    event::{self as crossterm_event, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend};
use std::time::Duration;

use crate::{
    app::{App, EntryOpener},
    error::AppResult,
};

use crate::core::GitWorker;
use bottom_bar::{format_metadata, render_bottom_bar};
use event::{
    is_cursor_down_event, is_cursor_up_event, is_enter_dir_event, is_enter_event, is_parent_event,
    is_quit_event, is_search_backspace_event, is_search_reset_event, is_toggle_hidden_event,
    search_char,
};
use layout::{split_main, split_panes};
use main_pane::render_entry_list;
use metadata_worker::MetadataWorker;
use top_bar::render_top_bar;

pub fn run(mut app: App, opener: &dyn EntryOpener) -> AppResult<()> {
    let mut guard = TerminalGuard::new()?;
    let metadata_worker = MetadataWorker::new();
    let git_worker = GitWorker::new();
    let mut last_metadata_path: Option<std::path::PathBuf> = None;
    let mut metadata_display: Option<String> = None;
    let mut last_git_dir: Option<std::path::PathBuf> = None;
    let mut git_display: Option<String> = None;

    loop {
        let current_path = app.selected_entry_path();
        while let Some(result) = metadata_worker.poll() {
            if Some(&result.path) != current_path.as_ref() {
                continue;
            }
            match result.metadata {
                Some(metadata) => {
                    metadata_display = Some(format_metadata(&metadata));
                }
                None => {
                    metadata_display = None;
                }
            };
        }
        if current_path != last_metadata_path {
            metadata_display = None;
            if let Some(path) = current_path.clone() {
                metadata_worker.request(path);
            }
            last_metadata_path = current_path;
        }
        let current_dir = app.current_dir.clone();
        while let Some(result) = git_worker.poll() {
            if result.path != current_dir {
                continue;
            }
            git_display = result.branch.map(|branch| format!("git: {branch}"));
        }
        if last_git_dir.as_ref() != Some(&current_dir) {
            git_display = None;
            git_worker.request(current_dir.clone());
            last_git_dir = Some(current_dir);
        }

        guard.terminal_mut().draw(|frame| {
            draw(
                frame,
                &app,
                metadata_display.as_deref(),
                git_display.as_deref(),
            )
        })?;

        if crossterm_event::poll(Duration::from_millis(200))? {
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
                if is_toggle_hidden_event(key) {
                    app.toggle_hidden()?;
                }
                if is_search_reset_event(key) {
                    app.reset_search();
                }
                if is_search_backspace_event(key) {
                    app.backspace_search_char();
                }
                if let Some(ch) = search_char(key) {
                    app.append_search_char(ch);
                }
            }
        }
    }

    Ok(())
}

fn draw(
    frame: &mut Frame<'_>,
    app: &App,
    metadata_display: Option<&str>,
    git_display: Option<&str>,
) {
    let area = frame.area();
    let (top, main, bottom) = split_main(area);
    render_top_bar(frame, top, app);
    let (left, right) = split_panes(main);
    render_entry_list(frame, left, &app.parent_entries, None, "parent", "");
    render_entry_list(
        frame,
        right,
        &app.entries,
        app.cursor,
        "current",
        app.search_text(),
    );
    render_bottom_bar(frame, bottom, metadata_display, git_display);
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
