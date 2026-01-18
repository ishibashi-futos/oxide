mod bottom_bar;
mod event;
mod layout;
mod main_pane;
mod metadata_worker;
mod preview_pane;
mod preview_worker;
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

use crate::core::{GitWorker, PreviewEvent, PreviewFailed, PreviewReady, PreviewRequest};
use bottom_bar::{format_metadata, render_bottom_bar, render_slash_bar};
use event::{
    is_cursor_down_event, is_cursor_up_event, is_enter_dir_event, is_enter_event, is_parent_event,
    is_quit_event, is_search_backspace_event, is_search_reset_event, is_slash_activate_event,
    is_slash_cancel_event, is_toggle_hidden_event, search_char, slash_input_char,
};
use layout::{split_main, split_panes};
use main_pane::render_entry_list;
use metadata_worker::MetadataWorker;
use preview_pane::{PreviewPaneState, render_preview_pane};
use preview_worker::PreviewWorker;
use top_bar::render_top_bar;

pub fn run(mut app: App, opener: &dyn EntryOpener) -> AppResult<()> {
    let mut guard = TerminalGuard::new()?;
    let metadata_worker = MetadataWorker::new();
    let git_worker = GitWorker::new();
    let preview_worker = PreviewWorker::new();
    let mut last_metadata_path: Option<std::path::PathBuf> = None;
    let mut metadata_display: Option<String> = None;
    let mut last_git_dir: Option<std::path::PathBuf> = None;
    let mut git_display: Option<String> = None;
    let mut last_preview_path: Option<std::path::PathBuf> = None;
    let mut preview_state = PreviewState::Idle;
    let mut preview_request_id: u64 = 0;
    let mut active_preview_id: Option<u64> = None;

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
            last_metadata_path = current_path.clone();
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
        while let Some(event) = preview_worker.poll() {
            if Some(preview_event_id(&event)) != active_preview_id {
                continue;
            }
            preview_state = match event {
                PreviewEvent::Loading { .. } => PreviewState::Loading,
                PreviewEvent::Ready(ready) => PreviewState::Ready(ready),
                PreviewEvent::Failed(failed) => PreviewState::Failed(failed),
            };
        }
        if app.preview_visible() {
            if current_path != last_preview_path {
                if let Some(path) = current_path.clone() {
                    preview_request_id += 1;
                    let id = preview_request_id;
                    active_preview_id = Some(id);
                    preview_state = PreviewState::Loading;
                    preview_worker.request(PreviewRequest {
                        id,
                        path,
                        max_bytes: 1024 * 1024,
                    });
                } else {
                    preview_state = PreviewState::Idle;
                    active_preview_id = None;
                }
                last_preview_path = current_path.clone();
            }
        } else {
            last_preview_path = None;
            preview_state = PreviewState::Idle;
            active_preview_id = None;
        }

        guard.terminal_mut().draw(|frame| {
            draw(
                frame,
                &app,
                metadata_display.as_deref(),
                git_display.as_deref(),
                &preview_state,
            )
        })?;

        if crossterm_event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = crossterm_event::read()?
        {
            if app.slash_input_active() {
                if is_slash_cancel_event(key) {
                    app.cancel_slash_input();
                    continue;
                }
                if is_enter_event(key) {
                    let _ = app.submit_slash_command();
                    continue;
                }
                if is_search_backspace_event(key) {
                    app.backspace_slash_char();
                    continue;
                }
                if let Some(ch) = slash_input_char(key) {
                    app.append_slash_char(ch);
                    continue;
                }
                continue;
            }
            if is_quit_event(key) {
                break;
            }
            if is_slash_activate_event(key) {
                app.activate_slash_input();
                continue;
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

    Ok(())
}

fn draw(
    frame: &mut Frame<'_>,
    app: &App,
    metadata_display: Option<&str>,
    git_display: Option<&str>,
    preview_state: &PreviewState,
) {
    let area = frame.area();
    let (top, main, bottom, slash) = split_main(area, app.slash_input_active());
    render_top_bar(frame, top, app);
    let (left, right, preview) = split_panes(main, app.preview_visible());
    render_entry_list(frame, left, &app.parent_entries, None, "parent", "");
    render_entry_list(
        frame,
        right,
        &app.entries,
        app.cursor,
        "current",
        app.search_text(),
    );
    if let Some(preview_area) = preview {
        let pane_state = match preview_state {
            PreviewState::Idle => PreviewPaneState::Empty,
            PreviewState::Loading => PreviewPaneState::Loading,
            PreviewState::Ready(ready) => PreviewPaneState::Ready {
                lines: &ready.lines,
                reason: ready.reason.clone(),
                truncated: ready.truncated,
            },
            PreviewState::Failed(failed) => PreviewPaneState::Failed {
                reason: preview_error_text(failed),
            },
        };
        render_preview_pane(frame, preview_area, pane_state);
    }
    render_bottom_bar(
        frame,
        bottom,
        metadata_display,
        git_display,
        app.slash_feedback(),
    );
    if let Some(slash_area) = slash {
        render_slash_bar(frame, slash_area, app.slash_input_text());
    }
}

#[derive(Debug, Clone)]
enum PreviewState {
    Idle,
    Loading,
    Ready(PreviewReady),
    Failed(PreviewFailed),
}

fn preview_event_id(event: &PreviewEvent) -> u64 {
    match event {
        PreviewEvent::Loading { id } => *id,
        PreviewEvent::Ready(ready) => ready.id,
        PreviewEvent::Failed(failed) => failed.id,
    }
}

fn preview_error_text(failed: &PreviewFailed) -> String {
    use crate::core::PreviewError;
    match &failed.reason {
        PreviewError::TooLarge => "preview: too large".to_string(),
        PreviewError::BinaryFile => "preview: binary file".to_string(),
        PreviewError::PermissionDenied => "preview: permission denied".to_string(),
        PreviewError::IoError(message) => format!("preview: {message}"),
    }
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
