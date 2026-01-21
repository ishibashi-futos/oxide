mod bottom_bar;
mod event;
mod layout;
mod main_pane;
mod metadata_worker;
mod preview_pane;
mod preview_worker;
mod theme;
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
    app::{App, EntryOpener, TabColorChanged},
    error::AppResult,
};

use crate::core::{
    FetchPriority, GitWorker, MetadataFetchResult, MetadataSnapshot, MetadataStatus,
    MetadataWindow, PreviewEvent, PreviewFailed, PreviewReady, PreviewRequest, RequestId,
    RequestTracker,
};
use bottom_bar::{format_metadata, render_bottom_bar, render_slash_bar};
use event::{
    is_cursor_down_event, is_cursor_up_event, is_enter_dir_event, is_enter_event, is_new_tab_event,
    is_next_tab_event, is_parent_event, is_prev_tab_event, is_quit_event,
    is_search_backspace_event, is_search_reset_event, is_slash_activate_event,
    is_slash_cancel_event, is_slash_complete_event, is_slash_history_next_event,
    is_slash_history_prev_event, is_toggle_hidden_event, search_char, slash_input_char,
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
    let mut metadata_status: Option<MetadataStatus> = None;
    let mut request_tracker = RequestTracker::new();
    let mut metadata_snapshot = MetadataSnapshot::new();
    let mut active_metadata_request: Option<RequestId> = None;
    let mut metadata_window: MetadataWindow<std::path::PathBuf> = MetadataWindow::new();
    let mut metadata_cache_dir: Option<std::path::PathBuf> = None;
    let mut last_git_dir: Option<std::path::PathBuf> = None;
    let mut git_display: Option<String> = None;
    let mut last_preview_path: Option<std::path::PathBuf> = None;
    let mut preview_state = PreviewState::Idle;
    let mut preview_request_id: u64 = 0;
    let mut active_preview_id: Option<u64> = None;
    let mut theme_state = ThemeState::new(app.active_theme());

    loop {
        if let Some(event) = app.take_tab_color_changed() {
            theme_state.apply(event);
        }
        let current_path = app.selected_entry_path();
        if metadata_cache_dir.as_ref() != Some(&app.current_dir) {
            metadata_snapshot.clear();
            metadata_cache_dir = Some(app.current_dir.clone());
        }
        while let Some(result) = metadata_worker.poll() {
            if !request_tracker.is_latest(result.request_id) {
                continue;
            }
            let is_selected = Some(&result.path) == current_path.as_ref();
            let mut metadata_for_display = None;
            let metadata_result = match result.metadata {
                Ok(metadata) => {
                    if is_selected {
                        metadata_for_display = Some(metadata.clone());
                    }
                    Ok(metadata)
                }
                Err(error) => {
                    if is_selected {
                        metadata_display = None;
                        metadata_status = Some(MetadataStatus::Error);
                    }
                    Err(error)
                }
            };
            metadata_snapshot.apply(MetadataFetchResult {
                request_id: result.request_id,
                path: result.path,
                metadata: metadata_result,
            });
            if let Some(metadata) = metadata_for_display {
                metadata_display = Some(format_metadata(&metadata));
                metadata_status = None;
            }
        }
        if metadata_display.is_none() {
            if let Some(path) = current_path.as_ref() {
                if let Some(metadata) = metadata_snapshot.get(path) {
                    metadata_display = Some(format_metadata(metadata));
                    metadata_status = None;
                }
            }
        }
        if current_path != last_metadata_path {
            metadata_display = None;
            if let Some(path) = current_path.clone() {
                let request_id = request_tracker.next();
                if let Some(previous) = active_metadata_request {
                    metadata_worker.cancel(previous);
                }
                active_metadata_request = Some(request_id);
                if metadata_snapshot.get(&path).is_some() {
                    metadata_status = None;
                } else {
                    metadata_status = Some(MetadataStatus::Loading);
                }
                let selected_index = app.cursor.unwrap_or(0);
                let paths: Vec<std::path::PathBuf> = app
                    .entries
                    .iter()
                    .map(|entry| app.current_dir.join(&entry.name))
                    .collect();
                let selected_path = path.clone();
                metadata_window.refresh(&paths, selected_index);
                for prefetch_path in metadata_window.items().iter().cloned() {
                    if metadata_snapshot.get(&prefetch_path).is_some() {
                        continue;
                    }
                    let priority = if prefetch_path == selected_path {
                        FetchPriority::High
                    } else {
                        FetchPriority::Low
                    };
                    metadata_worker.request(request_id, prefetch_path, priority);
                }
            } else {
                metadata_status = None;
                if let Some(previous) = active_metadata_request {
                    metadata_worker.cancel(previous);
                }
                active_metadata_request = None;
                metadata_window.refresh(&[], 0);
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
                metadata_status,
                git_display.as_deref(),
                &preview_state,
                &theme_state.current,
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
                if is_slash_history_prev_event(key) {
                    app.slash_history_prev();
                    continue;
                }
                if is_slash_history_next_event(key) {
                    app.slash_history_next();
                    continue;
                }
                if is_slash_complete_event(key) {
                    app.complete_slash_candidate();
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
            if is_new_tab_event(key) {
                app.new_tab()?;
                continue;
            }
            if is_next_tab_event(key) {
                app.next_tab()?;
                continue;
            }
            if is_prev_tab_event(key) {
                app.prev_tab()?;
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
    metadata_status: Option<MetadataStatus>,
    git_display: Option<&str>,
    preview_state: &PreviewState,
    theme: &crate::core::ColorTheme,
) {
    let area = frame.area();
    let (top, main, bottom, slash) = split_main(area, app.slash_input_active());
    render_top_bar(frame, top, app);
    let preview_ratio = if app.preview_visible() {
        Some(app.preview_ratio_percent())
    } else {
        None
    };
    let (left, right, preview) = split_panes(main, preview_ratio);
    render_entry_list(frame, left, &app.parent_entries, None, "parent", "", theme, false);
    render_entry_list(
        frame,
        right,
        &app.entries,
        app.cursor,
        "current",
        app.search_text(),
        theme,
        true,
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
        metadata_status,
        git_display,
        app.slash_feedback(),
        theme,
    );
    if let Some(slash_area) = slash {
        let candidates = app.slash_candidates();
        let hint = app.slash_hint();
        render_slash_bar(
            frame,
            slash_area,
            app.slash_input_text(),
            &candidates,
            hint.as_deref(),
            theme,
        );
    }
}

#[derive(Debug, Clone)]
enum PreviewState {
    Idle,
    Loading,
    Ready(PreviewReady),
    Failed(PreviewFailed),
}

struct ThemeState {
    current: crate::core::ColorTheme,
}

impl ThemeState {
    fn new(theme: crate::core::ColorTheme) -> Self {
        Self { current: theme }
    }

    fn apply(&mut self, event: TabColorChanged) {
        self.current = event.theme;
    }
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
        PreviewError::IoError(message) => {
            let lower = message.to_ascii_lowercase();
            if lower.contains("is a directory") {
                "no content: is a directory".to_string()
            } else {
                format!("preview: {message}")
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{PreviewError, PreviewFailed};

    #[test]
    fn preview_error_text_shows_directory_reason() {
        let failed = PreviewFailed {
            id: 1,
            reason: PreviewError::IoError("Is a directory".to_string()),
        };

        let text = preview_error_text(&failed);

        assert_eq!(text, "no content: is a directory");
    }
}
