use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::core::{
    ColorTheme, ColorThemeId, Entry, ShellCommandError, ShellCommandRequest, ShellEvent,
    ShellExecutionResult, ShellPermission, ShellWorker, SlashCommand, SlashCommandError,
    list_entries, parse_slash_command, restore_start_dir, save_session_async,
};
use crate::error::{AppError, AppResult};
use crate::tabs::{TabSummary, TabsState};

pub trait EntryOpener {
    fn open(&self, path: &Path) -> AppResult<()>;
}

pub trait AppClock: std::fmt::Debug {
    fn now(&self) -> Instant;
}

#[derive(Debug)]
struct SystemClock;

impl AppClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug)]
pub struct App {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub parent_entries: Vec<Entry>,
    pub cursor: Option<usize>,
    pub show_hidden: bool,
    clock: Arc<dyn AppClock>,
    tabs: TabsState,
    tab_color_changed: Option<TabColorChanged>,
    search_buffer: String,
    search_origin: Option<usize>,
    slash_input_active: bool,
    slash_input_buffer: String,
    slash_feedback: Option<SlashFeedback>,
    preview_visible: bool,
    preview_paused: bool,
    preview_ratio_percent: u16,
    slash_history: Vec<String>,
    slash_history_index: Option<usize>,
    shell_permission: ShellPermission,
    allow_opener: bool,
    shell_worker: ShellWorker,
    shell_output_view: ShellOutputView,
    shell_output_active: bool,
}

const SLASH_FEEDBACK_TTL: Duration = Duration::from_secs(4);

impl App {
    #[allow(dead_code)]
    // TODO: 設定注入やCLIオプション対応で使う想定のため保留。
    pub fn new(
        current_dir: PathBuf,
        entries: Vec<Entry>,
        parent_entries: Vec<Entry>,
        cursor: Option<usize>,
        show_hidden: bool,
    ) -> Self {
        Self::new_with_config(
            current_dir,
            entries,
            parent_entries,
            cursor,
            show_hidden,
            Config::default(),
        )
    }

    fn new_with_config(
        current_dir: PathBuf,
        entries: Vec<Entry>,
        parent_entries: Vec<Entry>,
        cursor: Option<usize>,
        show_hidden: bool,
        config: Config,
    ) -> Self {
        let clock = Arc::new(SystemClock);
        let tabs = TabsState::new(current_dir.clone(), config.default_theme);
        Self {
            current_dir,
            entries,
            parent_entries,
            cursor,
            show_hidden,
            clock,
            tabs,
            tab_color_changed: None,
            search_buffer: String::new(),
            search_origin: None,
            slash_input_active: false,
            slash_input_buffer: String::new(),
            slash_feedback: None,
            preview_visible: false,
            preview_paused: false,
            preview_ratio_percent: 35,
            slash_history: Vec::new(),
            slash_history_index: None,
            shell_permission: ShellPermission::from_env(config.allow_shell),
            allow_opener: config.allow_opener,
            shell_worker: ShellWorker::new(),
            shell_output_view: ShellOutputView::new(),
            shell_output_active: false,
        }
    }

    pub fn load(current_dir: PathBuf) -> AppResult<Self> {
        let show_hidden = false;
        let restored_dir = restore_start_dir(current_dir);
        let entries = list_entries(&restored_dir, show_hidden)?;
        let parent_entries = list_parent_entries(&restored_dir, show_hidden)?;
        let cursor = if entries.is_empty() { None } else { Some(0) };
        let config = Config::load();
        Ok(Self::new_with_config(
            restored_dir,
            entries,
            parent_entries,
            cursor,
            show_hidden,
            config,
        ))
    }

    pub fn move_cursor_up(&mut self) {
        let Some(cursor) = self.cursor else { return };
        if cursor == 0 {
            return;
        }
        self.cursor = Some(cursor - 1);
    }

    pub fn move_cursor_down(&mut self) {
        let Some(cursor) = self.cursor else { return };
        if cursor + 1 >= self.entries.len() {
            return;
        }
        self.cursor = Some(cursor + 1);
    }

    pub fn open_selected(&mut self, opener: &dyn EntryOpener) -> AppResult<()> {
        let Some(selected) = self.selected_entry() else {
            return Ok(());
        };
        let target = self.current_dir.join(&selected.name);
        if selected.is_dir {
            self.change_dir(target);
            self.refresh()?;
            return Ok(());
        }
        if !self.allow_opener {
            self.slash_feedback = Some(self.opener_disabled_feedback());
            return Ok(());
        }
        opener.open(&target)
    }

    pub fn enter_selected_dir(&mut self) -> AppResult<()> {
        let Some(selected) = self.selected_entry() else {
            return Ok(());
        };
        if !selected.is_dir {
            return Ok(());
        }
        let target = self.current_dir.join(&selected.name);
        self.change_dir(target);
        self.refresh()
    }

    pub fn move_to_parent(&mut self) -> AppResult<()> {
        let Some(parent) = self.current_dir.parent() else {
            return Ok(());
        };
        let focus_child = self
            .current_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string());
        self.change_dir(parent.to_path_buf());
        self.refresh_with_selection(focus_child.as_deref())
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.cursor.and_then(|index| self.entries.get(index))
    }

    pub fn selected_entry_path(&self) -> Option<PathBuf> {
        let selected = self.selected_entry()?;
        Some(self.current_dir.join(&selected.name))
    }

    pub fn toggle_hidden(&mut self) -> AppResult<()> {
        let selected_name = self.selected_entry().map(|entry| entry.name.clone());
        let selected_index = self.cursor;
        self.show_hidden = !self.show_hidden;
        self.reload_entries()?;
        self.cursor = resolve_cursor(&self.entries, selected_name.as_deref(), selected_index);
        Ok(())
    }

    pub fn search_text(&self) -> &str {
        &self.search_buffer
    }

    pub fn slash_input_active(&self) -> bool {
        self.slash_input_active
    }

    pub fn slash_input_text(&self) -> &str {
        &self.slash_input_buffer
    }

    pub fn slash_feedback(&self) -> Option<&SlashFeedback> {
        let feedback = self.slash_feedback.as_ref()?;
        if feedback.is_expired_with(self.clock.as_ref()) {
            return None;
        }
        Some(feedback)
    }

    pub fn shell_output_active(&self) -> bool {
        self.shell_output_active
    }

    pub fn shell_output_text(&self, height: usize, width: usize) -> Option<String> {
        if self.shell_output_view.is_empty() {
            None
        } else {
            Some(self.shell_output_view.view_text(height, width))
        }
    }

    pub fn page_up_shell_output(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.page_up();
        }
    }

    pub fn page_down_shell_output(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.page_down();
        }
    }

    pub fn home_shell_output(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.home();
        }
    }

    pub fn end_shell_output(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.end();
        }
    }

    pub fn scroll_shell_output_left(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.scroll_left();
        }
    }

    pub fn scroll_shell_output_right(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.scroll_right();
        }
    }

    pub fn preview_visible(&self) -> bool {
        self.preview_visible
    }

    pub fn preview_ratio_percent(&self) -> u16 {
        self.preview_ratio_percent
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.count()
    }

    pub fn active_tab_number(&self) -> usize {
        self.tabs.active_number()
    }

    pub fn active_theme(&self) -> ColorTheme {
        self.tabs.active_theme_id().theme()
    }

    pub fn take_tab_color_changed(&mut self) -> Option<TabColorChanged> {
        self.tab_color_changed.take()
    }

    pub fn new_tab(&mut self) -> AppResult<()> {
        self.tabs.push_new(self.current_dir.as_path());
        self.announce_active_tab_color();
        self.clear_search_state();
        self.request_session_save();
        Ok(())
    }

    pub fn next_tab(&mut self) -> AppResult<()> {
        let Some(next) = self.tabs.next_index() else {
            return Ok(());
        };
        self.switch_to_tab(next)
    }

    pub fn prev_tab(&mut self) -> AppResult<()> {
        let Some(prev) = self.tabs.prev_index() else {
            return Ok(());
        };
        self.switch_to_tab(prev)
    }

    pub fn activate_slash_input(&mut self) {
        self.slash_input_active = true;
        self.slash_input_buffer.clear();
        self.slash_input_buffer.push('/');
        self.slash_history_index = None;
    }

    pub fn cancel_slash_input(&mut self) {
        self.slash_input_active = false;
        self.slash_input_buffer.clear();
        self.slash_history_index = None;
    }

    pub fn append_slash_char(&mut self, ch: char) {
        if !self.slash_input_active {
            return;
        }
        self.slash_input_buffer.push(ch);
        self.slash_history_index = None;
    }

    pub fn backspace_slash_char(&mut self) {
        if !self.slash_input_active {
            return;
        }
        if self.slash_input_buffer.len() <= 1 {
            self.cancel_slash_input();
            return;
        }
        self.slash_input_buffer.pop();
        self.slash_history_index = None;
    }

    pub fn submit_slash_command(&mut self) -> Option<SlashCommand> {
        if !self.slash_input_active {
            return None;
        }
        let input_snapshot = self.slash_input_buffer.clone();
        let command = parse_slash_command(&self.slash_input_buffer);
        self.slash_input_active = false;
        self.slash_input_buffer.clear();
        self.slash_history_index = None;
        match command {
            Ok(command) => {
                self.slash_history.push(input_snapshot);
                self.slash_feedback = Some(self.handle_slash_command(&command));
                Some(command)
            }
            Err(error) => {
                self.slash_feedback = Some(self.timed_feedback(
                    format!("slash error: {}", format_slash_error(error)),
                    FeedbackStatus::Error,
                ));
                None
            }
        }
    }

    pub fn slash_history_prev(&mut self) {
        if self.slash_history.is_empty() {
            return;
        }
        let next_index = match self.slash_history_index {
            None => self.slash_history.len().saturating_sub(1),
            Some(index) => index.saturating_sub(1),
        };
        self.slash_history_index = Some(next_index);
        if let Some(entry) = self.slash_history.get(next_index) {
            self.slash_input_buffer = entry.clone();
        }
    }

    pub fn slash_history_next(&mut self) {
        let Some(index) = self.slash_history_index else {
            return;
        };
        let next_index = index.saturating_add(1);
        if next_index >= self.slash_history.len() {
            self.slash_history_index = None;
            self.slash_input_buffer = "/".to_string();
            return;
        }
        self.slash_history_index = Some(next_index);
        if let Some(entry) = self.slash_history.get(next_index) {
            self.slash_input_buffer = entry.clone();
        }
    }

    pub fn slash_candidates(&self) -> SlashCandidates {
        let Some(prefix) = self.slash_command_prefix() else {
            return SlashCandidates::default();
        };
        if prefix.is_empty() {
            return SlashCandidates::default();
        }
        if let Some(prefix) = self.color_candidate_prefix() {
            return SlashCandidates {
                items: color_theme_candidates(prefix),
            };
        }
        if let Some(target) = self.shell_completion_target() {
            return SlashCandidates {
                items: self.shell_path_candidates(&target.prefix),
            };
        }
        let items = slash_command_specs()
            .iter()
            .map(|spec| spec.name)
            .filter(|command| command.starts_with(prefix))
            .map(|command| SlashCandidate {
                text: format!("/{command}"),
                description: None,
            })
            .collect();
        SlashCandidates { items }
    }

    pub fn complete_slash_candidate(&mut self) {
        if let Some(target) = self.shell_completion_target() {
            let Some(candidate) = self
                .shell_path_candidates(&target.prefix)
                .into_iter()
                .next()
            else {
                return;
            };
            let mut buffer = String::new();
            buffer.push_str(&self.slash_input_buffer[..target.range.start]);
            buffer.push_str(&candidate.text);
            if !candidate.text.ends_with('/')
                && !candidate.text.ends_with(std::path::MAIN_SEPARATOR)
            {
                buffer.push(' ');
            }
            buffer.push_str(&self.slash_input_buffer[target.range.end..]);
            self.slash_input_buffer = buffer;
            self.slash_history_index = None;
            return;
        }
        let Some(candidate) = self.slash_candidates().items.into_iter().next() else {
            return;
        };
        if self.color_candidate_prefix().is_some() {
            self.slash_input_buffer = format!("/color {}", candidate.text);
        } else {
            if self.slash_input_buffer.contains(char::is_whitespace) {
                return;
            }
            self.slash_input_buffer = format!("{} ", candidate.text);
        }
        self.slash_history_index = None;
    }

    pub fn slash_hint(&self) -> Option<String> {
        if !self.slash_input_active {
            return None;
        }
        let trimmed = self.slash_input_buffer.trim_end();
        let stripped = trimmed.strip_prefix('/')?;
        let mut parts = stripped.split_whitespace();
        let name = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        let spec = slash_command_spec(name)?;
        if spec.options.is_empty() {
            return Some(spec.description.to_string());
        }
        Some(format!(
            "{} | options: {}",
            spec.description,
            spec.options.join(", ")
        ))
    }

    pub fn toggle_shell_output(&mut self) {
        if self.shell_output_view.is_empty() {
            return;
        }
        self.shell_output_active = !self.shell_output_active;
    }

    pub fn poll_shell_events(&mut self) {
        while let Some(event) = self.shell_worker.poll() {
            match event {
                ShellEvent::Stdout(line) => {
                    self.shell_output_view.append_stdout_line(&line);
                }
                ShellEvent::Stderr(line) => {
                    self.shell_output_view.append_stderr_line(&line);
                }
                ShellEvent::Finished(result) => {
                    self.shell_output_view.finish(&result);
                    self.slash_feedback = Some(self.shell_result_feedback(&result));
                }
                ShellEvent::Failed(error) => {
                    self.shell_output_view.append_stderr_line(&error);
                    self.slash_feedback = Some(self.timed_feedback(error, FeedbackStatus::Error));
                }
            }
        }
    }

    pub fn close_shell_output(&mut self) {
        self.shell_output_active = false;
    }

    pub fn scroll_shell_output_up(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.scroll_up();
        }
    }

    pub fn scroll_shell_output_down(&mut self) {
        if self.shell_output_active {
            self.shell_output_view.scroll_down();
        }
    }

    pub fn append_search_char(&mut self, ch: char) {
        if self.entries.is_empty() {
            return;
        }
        if self.search_buffer.is_empty() {
            self.search_origin = self.cursor;
        }
        self.search_buffer.push(ch);
        self.apply_search();
    }

    pub fn backspace_search_char(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }
        self.search_buffer.pop();
        if self.search_buffer.is_empty() {
            self.restore_search_origin();
            return;
        }
        self.apply_search();
    }

    pub fn reset_search(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }
        self.search_buffer.clear();
        self.restore_search_origin();
    }

    fn refresh(&mut self) -> AppResult<()> {
        self.refresh_with_selection(None)
    }

    fn reload_entries(&mut self) -> AppResult<()> {
        self.entries = list_entries(&self.current_dir, self.show_hidden)?;
        self.parent_entries = list_parent_entries(&self.current_dir, self.show_hidden)?;
        self.clear_search_state();
        Ok(())
    }

    fn refresh_with_selection(&mut self, focus_name: Option<&str>) -> AppResult<()> {
        self.reload_entries()?;
        self.cursor = resolve_cursor(&self.entries, focus_name, None);
        Ok(())
    }

    fn apply_search(&mut self) {
        if self.search_buffer.is_empty() {
            return;
        }
        let needle = self.search_buffer.as_str();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.name.starts_with(needle))
        {
            self.cursor = Some(index);
        }
    }

    fn clear_search_state(&mut self) {
        self.search_buffer.clear();
        self.search_origin = None;
    }

    fn restore_search_origin(&mut self) {
        let origin = self.search_origin.take();
        self.cursor = match origin {
            Some(index) => clamp_cursor(&self.entries, index),
            None => clamp_cursor(&self.entries, 0).or(self.cursor),
        };
    }

    fn set_current_dir(&mut self, path: PathBuf) {
        self.current_dir = path;
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.set_current_dir(path);
        self.store_active_tab();
        self.request_session_save();
    }

    fn store_active_tab(&mut self) {
        self.tabs.store_active(self.current_dir.as_path());
    }

    fn switch_to_tab(&mut self, index: usize) -> AppResult<()> {
        let Some(next_dir) = self.tabs.switch_to(index, self.current_dir.as_path()) else {
            return Ok(());
        };
        self.set_current_dir(next_dir);
        self.announce_active_tab_color();
        self.refresh()?;
        Ok(())
    }

    fn handle_slash_command(&mut self, command: &SlashCommand) -> SlashFeedback {
        match command.name.as_str() {
            "preview" => self.handle_preview_command(&command.args),
            "tab" => self.handle_tab_command(&command.args),
            "color" => self.handle_color_command(&command.args),
            "paste" => self.timed_feedback("paste: ready".to_string(), FeedbackStatus::Success),
            "shell" => self.handle_shell_command(command),
            _ => self.timed_feedback(
                format!("unknown command: {}", command.name),
                FeedbackStatus::Error,
            ),
        }
    }

    fn handle_preview_command(&mut self, args: &[String]) -> SlashFeedback {
        match args {
            [] => {
                let next = !self.preview_visible;
                self.preview_visible = next;
                self.preview_paused = !next;
                self.preview_feedback(next)
            }
            [arg] if arg == "show" => {
                self.preview_visible = true;
                self.preview_paused = false;
                self.preview_feedback(true)
            }
            [arg] if arg == "hide" => {
                self.preview_visible = false;
                self.preview_paused = true;
                self.preview_feedback(false)
            }
            _ => self.timed_feedback("preview: invalid args".to_string(), FeedbackStatus::Error),
        }
    }

    fn handle_tab_command(&mut self, args: &[String]) -> SlashFeedback {
        match args {
            [] => self.tab_list_feedback(),
            [arg] if arg == "new" => match self.new_tab() {
                Ok(()) => self.tab_list_feedback(),
                Err(error) => self.tab_error_feedback(error),
            },
            [arg] if arg == "next" => match self.next_tab() {
                Ok(()) => self.tab_list_feedback(),
                Err(error) => self.tab_error_feedback(error),
            },
            [arg] if arg == "prev" => match self.prev_tab() {
                Ok(()) => self.tab_list_feedback(),
                Err(error) => self.tab_error_feedback(error),
            },
            [arg] => match arg.parse::<usize>() {
                Ok(number) if number >= 1 && number <= self.tab_count() => {
                    match self.switch_to_tab(number.saturating_sub(1)) {
                        Ok(()) => self.tab_list_feedback(),
                        Err(error) => self.tab_error_feedback(error),
                    }
                }
                _ => self.timed_feedback("tab: invalid args".to_string(), FeedbackStatus::Error),
            },
            _ => self.timed_feedback("tab: invalid args".to_string(), FeedbackStatus::Error),
        }
    }

    fn handle_color_command(&mut self, args: &[String]) -> SlashFeedback {
        let name = args.join(" ").trim().to_string();
        if name.is_empty() {
            return self.timed_feedback("color: missing theme".to_string(), FeedbackStatus::Error);
        }
        let Some(theme_id) = ColorThemeId::from_name(&name) else {
            return self.timed_feedback("color: unknown theme".to_string(), FeedbackStatus::Error);
        };
        let context = CommandContext::Tab(self.tabs.active_tab_id());
        let preference = self.tabs.set_active_theme(theme_id);
        let theme = theme_id.theme();
        self.tab_color_changed = Some(TabColorChanged {
            tab_id: preference.tab_id,
            theme,
        });
        self.request_session_save();
        match context {
            CommandContext::Tab(_) => {
                self.timed_feedback(format!("theme: {}", theme.name), FeedbackStatus::Success)
            }
        }
    }

    fn handle_shell_command(&mut self, command: &SlashCommand) -> SlashFeedback {
        if !self.shell_permission.is_allowed() {
            return self
                .timed_feedback("Shell commands disabled".to_string(), FeedbackStatus::Warn);
        }
        let raw = command
            .raw
            .strip_prefix("/shell")
            .unwrap_or(command.raw.as_str())
            .trim_start();
        let request = match ShellCommandRequest::new(self.current_dir.clone(), raw) {
            Ok(request) => request,
            Err(error) => return self.shell_error_feedback(error),
        };
        self.shell_output_view.start(&request);
        self.shell_worker.request(request);
        self.timed_feedback("shell: running".to_string(), FeedbackStatus::Success)
    }

    fn tab_list_feedback(&self) -> SlashFeedback {
        let summaries = self.tabs.summaries();
        SlashFeedback {
            text: String::new(),
            status: FeedbackStatus::Success,
            tabs: Some(summaries),
            expires_at: self.clock.now() + SLASH_FEEDBACK_TTL,
        }
    }

    fn request_session_save(&mut self) {
        self.store_active_tab();
        let tabs = self.tabs.session_tabs();
        save_session_async(tabs);
    }

    fn slash_command_prefix(&self) -> Option<&str> {
        let mut parts = self.slash_input_buffer.split_whitespace();
        let command = parts.next()?;
        command.strip_prefix('/')
    }

    fn color_candidate_prefix(&self) -> Option<&str> {
        let trimmed = self.slash_input_buffer.trim_end();
        let stripped = trimmed.strip_prefix('/')?;
        let rest = stripped.strip_prefix("color")?;
        if rest.is_empty() {
            return Some("");
        }
        if rest.starts_with(char::is_whitespace) {
            return Some(rest.trim_start());
        }
        None
    }

    fn shell_completion_target(&self) -> Option<ShellCompletionTarget> {
        let input = self.slash_input_buffer.as_str();
        let after = input.strip_prefix("/shell")?;
        if after.is_empty() {
            return None;
        }
        if !after.starts_with(char::is_whitespace) {
            return None;
        }
        let has_trailing_space = input
            .chars()
            .last()
            .map(|ch| ch.is_whitespace())
            .unwrap_or(false);
        let (start, end) = if has_trailing_space {
            (input.len(), input.len())
        } else {
            let trimmed = input.trim_end();
            let end = trimmed.len();
            let start = trimmed
                .char_indices()
                .rev()
                .find(|(_, ch)| ch.is_whitespace())
                .map(|(idx, ch)| idx + ch.len_utf8())
                .unwrap_or(0);
            (start, end)
        };
        let prefix = if start == end {
            String::new()
        } else {
            input[start..end].to_string()
        };
        Some(ShellCompletionTarget {
            range: start..end,
            prefix,
        })
    }

    fn shell_path_candidates(&self, prefix: &str) -> Vec<SlashCandidate> {
        if prefix.contains('"') || prefix.contains('\'') {
            return Vec::new();
        }
        let (dir_prefix, name_prefix) = split_path_prefix(prefix);
        let base_dir = if Path::new(prefix).is_absolute() {
            PathBuf::from(&dir_prefix)
        } else {
            self.current_dir.join(&dir_prefix)
        };
        let entries = match list_entries(&base_dir, self.show_hidden) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };
        entries
            .into_iter()
            .filter(|entry| entry.name.starts_with(&name_prefix))
            .map(|entry| {
                let mut text = String::new();
                text.push_str(&dir_prefix);
                text.push_str(&entry.name);
                if entry.is_dir {
                    text.push('/');
                }
                SlashCandidate {
                    text,
                    description: None,
                }
            })
            .collect()
    }

    fn timed_feedback(&self, text: String, status: FeedbackStatus) -> SlashFeedback {
        timed_feedback_with(self.clock.as_ref(), text, status)
    }

    fn preview_feedback(&self, enabled: bool) -> SlashFeedback {
        self.timed_feedback(
            format!("preview: {}", if enabled { "on" } else { "off" }),
            FeedbackStatus::Success,
        )
    }

    fn opener_disabled_feedback(&self) -> SlashFeedback {
        self.timed_feedback(
            "opener is disabled. Set allow_opener = true to open with external apps.".to_string(),
            FeedbackStatus::Warn,
        )
    }

    fn tab_error_feedback(&self, error: AppError) -> SlashFeedback {
        self.timed_feedback(format!("tab: {}", error), FeedbackStatus::Error)
    }

    fn shell_error_feedback(&self, error: ShellCommandError) -> SlashFeedback {
        self.timed_feedback(error.to_string(), FeedbackStatus::Error)
    }

    fn shell_result_feedback(&self, result: &ShellExecutionResult) -> SlashFeedback {
        let status = match result.status_code {
            Some(0) => FeedbackStatus::Success,
            _ => FeedbackStatus::Error,
        };
        self.timed_feedback(format_shell_result(result), status)
    }

    fn announce_active_tab_color(&mut self) {
        let theme_id = self.tabs.active_theme_id();
        self.tab_color_changed = Some(TabColorChanged {
            tab_id: self.tabs.active_tab_id(),
            theme: theme_id.theme(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabColorChanged {
    pub tab_id: u64,
    pub theme: ColorTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandContext {
    Tab(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellCompletionTarget {
    range: std::ops::Range<usize>,
    prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCandidate {
    pub text: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SlashCandidates {
    pub items: Vec<SlashCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashFeedback {
    pub text: String,
    pub status: FeedbackStatus,
    pub tabs: Option<Vec<TabSummary>>,
    pub(crate) expires_at: Instant,
}

impl SlashFeedback {
    fn is_expired_with(&self, clock: &dyn AppClock) -> bool {
        clock.now() > self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackStatus {
    Success,
    Error,
    Warn,
}

fn format_slash_error(error: SlashCommandError) -> &'static str {
    match error {
        SlashCommandError::MissingSlash => "missing '/'",
        SlashCommandError::MissingName => "missing command",
    }
}

fn timed_feedback_with(
    clock: &dyn AppClock,
    text: String,
    status: FeedbackStatus,
) -> SlashFeedback {
    SlashFeedback {
        text,
        status,
        tabs: None,
        expires_at: clock.now() + SLASH_FEEDBACK_TTL,
    }
}

struct SlashCommandSpec {
    name: &'static str,
    description: &'static str,
    options: &'static [&'static str],
}

fn slash_command_specs() -> &'static [SlashCommandSpec] {
    &[
        SlashCommandSpec {
            name: "tab",
            description: "tabs",
            options: &["new", "next", "prev", "<number>"],
        },
        SlashCommandSpec {
            name: "preview",
            description: "toggle preview",
            options: &["show", "hide"],
        },
        SlashCommandSpec {
            name: "shell",
            description: "run shell command",
            options: &["<command>"],
        },
        SlashCommandSpec {
            name: "color",
            description: "set tab theme",
            options: &["<theme>"],
        },
        SlashCommandSpec {
            name: "paste",
            description: "paste from clipboard",
            options: &[],
        },
    ]
}

fn slash_command_spec(name: &str) -> Option<&'static SlashCommandSpec> {
    slash_command_specs().iter().find(|spec| spec.name == name)
}

fn color_theme_candidates(prefix: &str) -> Vec<SlashCandidate> {
    let normalized = prefix.trim().to_ascii_lowercase();
    ColorThemeId::all()
        .iter()
        .copied()
        .map(ColorThemeId::theme)
        .filter(|theme| {
            if normalized.is_empty() {
                true
            } else {
                theme.name.to_ascii_lowercase().starts_with(&normalized)
            }
        })
        .map(|theme| SlashCandidate {
            text: theme.name.to_string(),
            description: Some(format!(
                "success {} warn {} error {} info {}",
                hex_color(theme.semantic.success),
                hex_color(theme.semantic.warn),
                hex_color(theme.semantic.error),
                hex_color(theme.semantic.info)
            )),
        })
        .collect()
}

fn hex_color(color: crate::core::ColorRgb) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn split_path_prefix(raw: &str) -> (String, String) {
    if raw.is_empty() {
        return (String::new(), String::new());
    }
    let separator = std::path::MAIN_SEPARATOR;
    if raw.ends_with('/') || raw.ends_with(separator) {
        return (raw.to_string(), String::new());
    }
    let separators = ['/', separator];
    if let Some(index) = raw.rfind(|ch| separators.contains(&ch)) {
        let (dir, file) = raw.split_at(index + 1);
        return (dir.to_string(), file.to_string());
    }
    (String::new(), raw.to_string())
}

#[derive(Debug, Clone)]
struct ShellOutputView {
    lines: VecDeque<String>,
    total_bytes: usize,
    max_bytes: usize,
    max_lines: usize,
    scroll_offset: usize,
    horiz_offset: usize,
    stdout_opened: bool,
    stderr_opened: bool,
}

impl ShellOutputView {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            total_bytes: 0,
            max_bytes: 2_097_152,
            max_lines: 4_000,
            scroll_offset: 0,
            horiz_offset: 0,
            stdout_opened: false,
            stderr_opened: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[cfg(test)]
    fn reset(&mut self, request: &ShellCommandRequest, result: &ShellExecutionResult) {
        self.lines.clear();
        self.total_bytes = 0;
        self.scroll_offset = 0;
        self.horiz_offset = 0;
        self.stdout_opened = false;
        self.stderr_opened = false;
        self.push_line(format!("$ {}", request.raw_command));
        self.push_line(format!("cwd: {}", request.working_dir.display()));
        let exit = result.status_code.unwrap_or(-1);
        self.push_line(format!(
            "exit: {} duration: {}ms timestamp: {}",
            exit, result.duration_ms, result.timestamp_ms
        ));
        if !result.stdout.trim().is_empty() {
            self.open_stdout();
            self.push_text(&result.stdout);
        }
        if !result.stderr.trim().is_empty() {
            self.open_stderr();
            self.push_text(&result.stderr);
        }
        self.shrink_to_limits();
    }

    fn start(&mut self, request: &ShellCommandRequest) {
        self.lines.clear();
        self.total_bytes = 0;
        self.scroll_offset = 0;
        self.horiz_offset = 0;
        self.stdout_opened = false;
        self.stderr_opened = false;
        self.push_line(format!("$ {}", request.raw_command));
        self.push_line(format!("cwd: {}", request.working_dir.display()));
        self.push_line("status: running".to_string());
    }

    fn append_stdout_line(&mut self, line: &str) {
        self.open_stdout();
        self.push_line(line.to_string());
    }

    fn append_stderr_line(&mut self, line: &str) {
        self.open_stderr();
        self.push_line(line.to_string());
    }

    fn finish(&mut self, result: &ShellExecutionResult) {
        let exit = result.status_code.unwrap_or(-1);
        self.push_line(format!(
            "exit: {} duration: {}ms timestamp: {}",
            exit, result.duration_ms, result.timestamp_ms
        ));
        if !self.stdout_opened && !result.stdout.trim().is_empty() {
            self.open_stdout();
            self.push_text(&result.stdout);
        }
        if !self.stderr_opened && !result.stderr.trim().is_empty() {
            self.open_stderr();
            self.push_text(&result.stderr);
        }
        self.shrink_to_limits();
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        let max_offset = self.lines.len().saturating_sub(1);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    fn scroll_left(&mut self) {
        self.horiz_offset = self.horiz_offset.saturating_sub(self.horiz_step());
    }

    fn scroll_right(&mut self) {
        let max_offset = self.max_horiz_offset();
        let next = self.horiz_offset + self.horiz_step();
        self.horiz_offset = next.min(max_offset);
    }

    fn page_up(&mut self) {
        let step = self.page_step();
        self.scroll_offset = self.scroll_offset.saturating_sub(step);
    }

    fn page_down(&mut self) {
        let step = self.page_step();
        let max_offset = self.lines.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + step).min(max_offset);
    }

    fn home(&mut self) {
        self.scroll_offset = 0;
    }

    fn end(&mut self) {
        self.scroll_offset = self.lines.len().saturating_sub(1);
    }

    fn view_text(&self, height: usize, width: usize) -> String {
        if self.lines.is_empty() {
            return "shell output: empty".to_string();
        }
        let start = self.scroll_offset.min(self.lines.len());
        let end = (start + height).min(self.lines.len());
        self.lines
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .map(|line| slice_line(line, self.horiz_offset, width))
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn push_text(&mut self, text: &str) {
        for line in text.lines() {
            self.push_line(line.to_string());
        }
    }

    fn push_line(&mut self, line: String) {
        self.total_bytes += line.len() + 1;
        self.lines.push_back(line);
        self.shrink_to_limits();
    }

    fn open_stdout(&mut self) {
        if !self.stdout_opened {
            self.push_line("stdout:".to_string());
            self.stdout_opened = true;
        }
    }

    fn open_stderr(&mut self) {
        if !self.stderr_opened {
            self.push_line("stderr:".to_string());
            self.stderr_opened = true;
        }
    }

    fn shrink_to_limits(&mut self) {
        while self.lines.len() > self.max_lines || self.total_bytes > self.max_bytes {
            if let Some(line) = self.lines.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(line.len() + 1);
            } else {
                break;
            }
        }
        if self.scroll_offset >= self.lines.len() {
            self.scroll_offset = self.lines.len().saturating_sub(1);
        }
        if self.horiz_offset > self.max_horiz_offset() {
            self.horiz_offset = self.max_horiz_offset();
        }
    }

    fn page_step(&self) -> usize {
        20
    }

    fn horiz_step(&self) -> usize {
        4
    }

    fn max_horiz_offset(&self) -> usize {
        self.lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    }
}

fn slice_line(line: &str, offset: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    line.chars().skip(offset).take(width).collect()
}

fn format_shell_result(result: &ShellExecutionResult) -> String {
    let exit = result.status_code.unwrap_or(-1);
    let mut parts = vec![
        format!("shell: exit={exit}"),
        format!("{}ms", result.duration_ms),
    ];
    if let Some(stdout) = compact_output(&result.stdout, 80) {
        parts.push(format!("stdout={stdout}"));
    }
    if let Some(stderr) = compact_output(&result.stderr, 80) {
        parts.push(format!("stderr={stderr}"));
    }
    parts.join(" ")
}

fn compact_output(output: &str, max_len: usize) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut flat = trimmed.replace('\n', "\\n");
    if flat.len() > max_len {
        flat.truncate(max_len);
        flat.push_str("...");
    }
    Some(flat)
}

#[cfg(test)]
mod slash_tests {
    use super::*;
    use crate::tabs::TabSummary;

    #[derive(Debug)]
    struct FixedClock {
        now: Instant,
    }

    impl AppClock for FixedClock {
        fn now(&self) -> Instant {
            self.now
        }
    }

    fn empty_app() -> App {
        App::new(PathBuf::from("."), Vec::new(), Vec::new(), None, false)
    }

    fn app_with_two_tabs() -> (tempfile::TempDir, App, PathBuf, PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_one = temp_dir.path().join("one");
        let dir_two = temp_dir.path().join("two");
        std::fs::create_dir(&dir_one).unwrap();
        std::fs::create_dir(&dir_two).unwrap();

        let mut app = App::load(dir_one.clone()).unwrap();
        app.new_tab().unwrap();
        app.change_dir(dir_two.clone());

        (temp_dir, app, dir_one, dir_two)
    }

    #[test]
    fn activate_slash_input_sets_prompt() {
        let mut app = empty_app();

        app.activate_slash_input();

        assert!(app.slash_input_active());
        assert_eq!(app.slash_input_text(), "/");
    }

    #[test]
    fn cancel_slash_input_clears_buffer() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');

        app.cancel_slash_input();

        assert!(!app.slash_input_active());
        assert_eq!(app.slash_input_text(), "");
    }

    #[test]
    fn backspace_slash_input_exits_when_empty() {
        let mut app = empty_app();
        app.activate_slash_input();

        app.backspace_slash_char();

        assert!(!app.slash_input_active());
        assert_eq!(app.slash_input_text(), "");
    }

    #[test]
    fn submit_slash_command_parses_and_deactivates() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');

        let command = app.submit_slash_command().unwrap();

        assert_eq!(command.name, "preview");
        assert_eq!(command.args.len(), 0);
        assert!(!app.slash_input_active());
        assert_eq!(app.slash_input_text(), "");
        assert_eq!(
            app.slash_feedback().unwrap().status,
            FeedbackStatus::Success
        );
    }

    #[test]
    fn preview_toggle_turns_off_when_visible() {
        let mut app = empty_app();
        app.preview_visible = true;
        app.preview_paused = false;

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "preview".to_string(),
            args: Vec::new(),
            raw: "/preview".to_string(),
        });

        assert!(!app.preview_visible());
        assert!(app.preview_paused);
        assert_eq!(feedback.text, "preview: off");
    }

    #[test]
    fn preview_show_turns_on_and_resumes() {
        let mut app = empty_app();
        app.preview_visible = false;
        app.preview_paused = true;

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "preview".to_string(),
            args: vec!["show".to_string()],
            raw: "/preview show".to_string(),
        });

        assert!(app.preview_visible());
        assert!(!app.preview_paused);
        assert_eq!(feedback.text, "preview: on");
    }

    #[test]
    fn preview_hide_turns_off_and_pauses() {
        let mut app = empty_app();
        app.preview_visible = true;
        app.preview_paused = false;

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "preview".to_string(),
            args: vec!["hide".to_string()],
            raw: "/preview hide".to_string(),
        });

        assert!(!app.preview_visible());
        assert!(app.preview_paused);
        assert_eq!(feedback.text, "preview: off");
    }

    #[test]
    fn slash_history_moves_through_entries() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');
        app.submit_slash_command();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');
        app.append_slash_char(' ');
        app.append_slash_char('s');
        app.append_slash_char('h');
        app.append_slash_char('o');
        app.append_slash_char('w');
        app.submit_slash_command();

        app.activate_slash_input();
        app.slash_history_prev();
        assert_eq!(app.slash_input_text(), "/preview show");
        app.slash_history_prev();
        assert_eq!(app.slash_input_text(), "/preview");
        app.slash_history_next();
        assert_eq!(app.slash_input_text(), "/preview show");
    }

    #[test]
    fn slash_candidates_filter_by_prefix() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');

        let candidates = app.slash_candidates();
        let texts = candidates
            .items
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<String>>();

        assert_eq!(texts, vec!["/preview".to_string(), "/paste".to_string()]);
    }

    #[test]
    fn slash_completion_uses_first_candidate() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');

        app.complete_slash_candidate();

        assert_eq!(app.slash_input_text(), "/preview ");
    }

    #[test]
    fn slash_completion_skips_when_args_present() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');
        app.append_slash_char(' ');
        app.append_slash_char('h');

        app.complete_slash_candidate();

        assert_eq!(app.slash_input_text(), "/preview h");
    }

    #[test]
    fn shell_candidates_target_last_token_only() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("build.sh"), "echo").unwrap();
        std::fs::create_dir(temp_dir.path().join("dir")).unwrap();
        std::fs::write(temp_dir.path().join("diary.txt"), "note").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.slash_input_active = true;
        app.slash_input_buffer = "/shell ./build.sh ./di".to_string();

        let candidates = app.slash_candidates();
        let texts = candidates
            .items
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<String>>();

        assert_eq!(texts, vec!["./diary.txt".to_string(), "./dir/".to_string()]);
    }

    #[test]
    fn shell_completion_replaces_last_token_and_appends_space() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp_dir.path().join("dir")).unwrap();
        std::fs::write(temp_dir.path().join("diary.txt"), "note").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.slash_input_active = true;
        app.slash_input_buffer = "/shell ./build.sh ./di".to_string();

        app.complete_slash_candidate();

        assert_eq!(app.slash_input_text(), "/shell ./build.sh ./diary.txt ");
    }

    #[test]
    fn shell_completion_skips_space_for_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp_dir.path().join("src")).unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.slash_input_active = true;
        app.slash_input_buffer = "/shell ./sr".to_string();

        app.complete_slash_candidate();

        assert_eq!(app.slash_input_text(), "/shell ./src/");
    }

    #[test]
    fn shell_completion_keeps_existing_shell_output() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("diary.txt"), "note").unwrap();
        let request = ShellCommandRequest::new(temp_dir.path().to_path_buf(), "echo hi").unwrap();
        let result = ShellExecutionResult {
            status_code: Some(0),
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 12,
            timestamp_ms: 1,
        };

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.shell_output_view.reset(&request, &result);
        let before = app.shell_output_view.lines.clone();
        app.slash_input_active = true;
        app.slash_input_buffer = "/shell ./di".to_string();

        app.complete_slash_candidate();

        assert_eq!(app.shell_output_view.lines, before);
    }

    #[test]
    fn color_command_sets_theme_for_active_tab() {
        let mut app = empty_app();

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "color".to_string(),
            args: vec!["Glacier".to_string(), "Coast".to_string()],
            raw: "/color Glacier Coast".to_string(),
        });

        assert_eq!(feedback.status, FeedbackStatus::Success);
        assert_eq!(feedback.text, "theme: Glacier Coast");
        assert_eq!(app.tabs.active_theme_id(), ColorThemeId::GlacierCoast);
    }

    #[test]
    fn color_command_rejects_unknown_theme() {
        let mut app = empty_app();

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "color".to_string(),
            args: vec!["Unknown".to_string()],
            raw: "/color Unknown".to_string(),
        });

        assert_eq!(feedback.status, FeedbackStatus::Error);
        assert!(feedback.text.contains("unknown"));
    }

    #[test]
    fn slash_candidates_show_color_themes_with_descriptions() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('c');
        app.append_slash_char('o');
        app.append_slash_char('l');
        app.append_slash_char('o');
        app.append_slash_char('r');
        app.append_slash_char(' ');

        let candidates = app.slash_candidates();
        let first = candidates.items.first().unwrap();

        assert!(first.text.contains("Glacier"));
        assert!(
            first
                .description
                .as_deref()
                .unwrap_or("")
                .contains("success")
        );
    }

    #[test]
    fn slash_hint_shows_description_and_options() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');

        let hint = app.slash_hint().unwrap();

        assert_eq!(hint, "toggle preview | options: show, hide");
    }

    #[test]
    fn slash_hint_hides_when_args_present() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('p');
        app.append_slash_char('r');
        app.append_slash_char('e');
        app.append_slash_char('v');
        app.append_slash_char('i');
        app.append_slash_char('e');
        app.append_slash_char('w');
        app.append_slash_char(' ');
        app.append_slash_char('s');

        let hint = app.slash_hint();

        assert!(hint.is_none());
    }

    #[test]
    fn slash_feedback_expires_after_ttl() {
        let mut app = empty_app();
        app.slash_feedback = Some(SlashFeedback {
            text: "preview: on".to_string(),
            status: FeedbackStatus::Success,
            tabs: None,
            expires_at: std::time::Instant::now() - std::time::Duration::from_secs(1),
        });

        assert!(app.slash_feedback().is_none());
    }

    #[test]
    fn slash_feedback_uses_injected_clock() {
        let now = Instant::now();
        let feedback = SlashFeedback {
            text: "preview: on".to_string(),
            status: FeedbackStatus::Success,
            tabs: None,
            expires_at: now + std::time::Duration::from_secs(1),
        };
        let clock = FixedClock {
            now: now + std::time::Duration::from_secs(2),
        };

        assert!(feedback.is_expired_with(&clock));
    }

    #[test]
    fn slash_hint_shows_tab_options() {
        let mut app = empty_app();
        app.activate_slash_input();
        app.append_slash_char('t');
        app.append_slash_char('a');
        app.append_slash_char('b');

        let hint = app.slash_hint().unwrap();

        assert_eq!(hint, "tabs | options: new, next, prev, <number>");
    }

    #[test]
    fn shell_command_warns_when_disabled() {
        let mut app = empty_app();
        app.shell_permission = ShellPermission::new(false, false);

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "shell".to_string(),
            args: vec!["echo".to_string()],
            raw: "/shell echo".to_string(),
        });

        assert_eq!(feedback.status, FeedbackStatus::Warn);
        assert!(feedback.text.contains("disabled"));
    }

    #[test]
    fn shell_output_view_resets_and_scrolls() {
        let request = ShellCommandRequest::new(PathBuf::from("."), "echo hi").unwrap();
        let result = ShellExecutionResult {
            status_code: Some(0),
            stdout: "line1\nline2".to_string(),
            stderr: String::new(),
            duration_ms: 5,
            timestamp_ms: 10,
        };
        let mut view = ShellOutputView::new();

        view.reset(&request, &result);
        assert!(!view.is_empty());

        let text = view.view_text(1, 80);
        assert!(text.contains("$ echo hi"));

        view.scroll_down();
        let scrolled = view.view_text(1, 80);
        assert_ne!(text, scrolled);
    }

    #[test]
    fn shell_output_view_pages_and_jumps() {
        let mut view = ShellOutputView::new();
        view.max_lines = 100;
        for i in 0..50 {
            view.push_line(format!("line {i}"));
        }

        view.page_down();
        let after_page_down = view.scroll_offset;
        assert!(after_page_down > 0);

        view.page_up();
        assert!(view.scroll_offset < after_page_down);

        view.end();
        assert_eq!(view.scroll_offset, view.lines.len().saturating_sub(1));

        view.home();
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn shell_output_view_scrolls_horizontally() {
        let mut view = ShellOutputView::new();
        view.push_line("abcdefghij".to_string());

        let first = view.view_text(1, 4);
        assert_eq!(first, "abcd");

        view.scroll_right();
        let scrolled = view.view_text(1, 4);
        assert_eq!(scrolled, "efgh");

        view.scroll_left();
        let back = view.view_text(1, 4);
        assert_eq!(back, "abcd");
    }

    #[test]
    fn tab_command_lists_tabs() {
        let (_temp_dir, mut app, dir_one, dir_two) = app_with_two_tabs();

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "tab".to_string(),
            args: Vec::new(),
            raw: "/tab".to_string(),
        });

        assert_eq!(feedback.status, FeedbackStatus::Success);
        assert_eq!(
            feedback.tabs,
            Some(vec![
                TabSummary {
                    number: 1,
                    path: dir_one,
                    active: false,
                },
                TabSummary {
                    number: 2,
                    path: dir_two,
                    active: true,
                },
            ])
        );
    }

    #[test]
    fn tab_command_switches_by_number() {
        let (_temp_dir, mut app, dir_one, _dir_two) = app_with_two_tabs();

        let feedback = app.handle_slash_command(&SlashCommand {
            name: "tab".to_string(),
            args: vec!["1".to_string()],
            raw: "/tab 1".to_string(),
        });

        assert_eq!(feedback.status, FeedbackStatus::Success);
        assert_eq!(app.current_dir, dir_one);
        assert_eq!(app.active_tab_number(), 1);
    }

    #[test]
    fn preview_ratio_is_preserved_between_toggle() {
        let mut app = empty_app();
        app.preview_ratio_percent = 32;
        app.preview_visible = true;

        let _ = app.handle_slash_command(&SlashCommand {
            name: "preview".to_string(),
            args: Vec::new(),
            raw: "/preview".to_string(),
        });
        let _ = app.handle_slash_command(&SlashCommand {
            name: "preview".to_string(),
            args: Vec::new(),
            raw: "/preview".to_string(),
        });

        assert_eq!(app.preview_ratio_percent(), 32);
    }
}

fn list_parent_entries(current_dir: &Path, show_hidden: bool) -> AppResult<Vec<Entry>> {
    let Some(parent) = current_dir.parent() else {
        return Ok(Vec::new());
    };
    list_entries(parent, show_hidden)
}

fn resolve_cursor(
    entries: &[Entry],
    selected_name: Option<&str>,
    selected_index: Option<usize>,
) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }
    if let Some(name) = selected_name {
        if let Some(index) = entries.iter().position(|entry| entry.name == name) {
            return Some(index);
        }
        if let Some(index) = selected_index {
            return Some(index.saturating_sub(1).min(entries.len().saturating_sub(1)));
        }
    }
    Some(0)
}

fn clamp_cursor(entries: &[Entry], index: usize) -> Option<usize> {
    if entries.is_empty() {
        None
    } else {
        Some(index.min(entries.len().saturating_sub(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_cursor_up_stops_at_top() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_up_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
            false,
        );

        app.move_cursor_up();

        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn move_cursor_down_stops_at_bottom() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
            false,
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn move_cursor_down_moves_one_step() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "a.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "b.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.move_cursor_down();

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn open_selected_enters_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let child_dir = temp_dir.path().join("child");
        std::fs::create_dir(&child_dir).unwrap();

        let entries = vec![Entry {
            name: "child".to_string(),
            is_dir: true,
        }];
        let mut app = App::new(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
        );

        let opener = RecordingOpener::default();
        app.open_selected(&opener).unwrap();

        assert_eq!(app.current_dir, child_dir);
        assert!(opener.opened_paths.borrow().is_empty());
    }

    #[test]
    fn open_selected_uses_opener_for_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();

        let entries = vec![Entry {
            name: "note.txt".to_string(),
            is_dir: false,
        }];
        let config = Config {
            default_theme: None,
            allow_shell: false,
            allow_opener: true,
        };
        let mut app = App::new_with_config(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
            config,
        );

        let opener = RecordingOpener::default();
        app.open_selected(&opener).unwrap();

        assert_eq!(
            opener.opened_paths.borrow().as_slice(),
            std::slice::from_ref(&file)
        );
    }

    #[test]
    fn open_selected_warns_when_opener_is_disabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();

        let entries = vec![Entry {
            name: "note.txt".to_string(),
            is_dir: false,
        }];
        let config = Config {
            default_theme: None,
            allow_shell: false,
            allow_opener: false,
        };
        let mut app = App::new_with_config(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
            config,
        );

        let opener = RecordingOpener::default();
        app.open_selected(&opener).unwrap();

        assert!(opener.opened_paths.borrow().is_empty());
        let feedback = app.slash_feedback().unwrap();
        assert_eq!(feedback.status, FeedbackStatus::Warn);
        assert_eq!(
            feedback.text,
            "opener is disabled. Set allow_opener = true to open with external apps."
        );
    }

    #[test]
    fn enter_selected_dir_skips_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("note.txt");
        std::fs::write(&file, "hi").unwrap();

        let entries = vec![Entry {
            name: "note.txt".to_string(),
            is_dir: false,
        }];
        let mut app = App::new(
            temp_dir.path().to_path_buf(),
            entries,
            Vec::new(),
            Some(0),
            false,
        );

        app.enter_selected_dir().unwrap();

        assert_eq!(app.current_dir, temp_dir.path());
    }

    #[test]
    fn move_to_parent_updates_current_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let child_dir = temp_dir.path().join("child");
        std::fs::create_dir(&child_dir).unwrap();

        let mut app = App::load(child_dir).unwrap();

        app.move_to_parent().unwrap();

        assert_eq!(app.current_dir, temp_dir.path());
    }

    #[test]
    fn move_to_parent_focuses_child_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let parent_dir = temp_dir.path().join("parent");
        std::fs::create_dir(&parent_dir).unwrap();
        let other = parent_dir.join("alpha");
        let target = parent_dir.join("target");
        std::fs::create_dir(&other).unwrap();
        std::fs::create_dir(&target).unwrap();

        let mut app = App::load(target.clone()).unwrap();

        app.move_to_parent().unwrap();

        assert_eq!(app.current_dir, parent_dir);
        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("target")
        );
    }

    #[test]
    fn toggle_hidden_refreshes_entries() {
        let temp_dir = tempfile::tempdir().unwrap();
        let hidden = temp_dir.path().join(".secret");
        std::fs::write(&hidden, "hidden").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        assert!(app.entries.is_empty());

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.entries,
            vec![Entry {
                name: ".secret".to_string(),
                is_dir: false,
            }]
        );
    }

    #[test]
    fn toggle_hidden_keeps_cursor_on_same_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(".secret"), "hidden").unwrap();
        std::fs::write(temp_dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(temp_dir.path().join("b.txt"), "b").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.cursor = Some(1);

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("b.txt")
        );
    }

    #[test]
    fn toggle_hidden_moves_to_previous_when_hidden_selected() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join(".secret"), "hidden").unwrap();
        std::fs::write(temp_dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(temp_dir.path().join("b.txt"), "b").unwrap();

        let mut app = App::load(temp_dir.path().to_path_buf()).unwrap();
        app.toggle_hidden().unwrap();
        app.cursor = Some(0);

        app.toggle_hidden().unwrap();

        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("a.txt")
        );
    }

    #[test]
    fn incremental_search_moves_cursor_and_resets() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
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
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.append_search_char('b');
        assert_eq!(app.cursor, Some(1));

        app.append_search_char('r');
        assert_eq!(app.cursor, Some(2));

        app.reset_search();
        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn incremental_search_keeps_cursor_when_no_match() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
                Entry {
                    name: "alpha.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "beta.txt".to_string(),
                    is_dir: false,
                },
            ],
            Vec::new(),
            Some(1),
            false,
        );

        app.append_search_char('z');

        assert_eq!(app.cursor, Some(1));
    }

    #[test]
    fn incremental_search_backspace_restores_origin() {
        let mut app = App::new(
            PathBuf::from("."),
            vec![
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
            ],
            Vec::new(),
            Some(0),
            false,
        );

        app.append_search_char('b');
        app.append_search_char('r');
        assert_eq!(app.cursor, Some(2));

        app.backspace_search_char();
        assert_eq!(app.cursor, Some(1));

        app.backspace_search_char();
        assert_eq!(app.cursor, Some(0));
    }

    #[test]
    fn new_tab_adds_and_selects() {
        let mut app = App::new(PathBuf::from("/tmp"), Vec::new(), Vec::new(), None, false);

        app.new_tab().unwrap();

        assert_eq!(app.tab_count(), 2);
        assert_eq!(app.active_tab_number(), 2);
    }

    #[test]
    fn switching_tabs_restores_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_one = temp_dir.path().join("one");
        let dir_two = temp_dir.path().join("two");
        std::fs::create_dir(&dir_one).unwrap();
        std::fs::create_dir(&dir_two).unwrap();

        let mut app = App::load(dir_one.clone()).unwrap();
        app.new_tab().unwrap();

        app.change_dir(dir_two.clone());

        app.prev_tab().unwrap();
        assert_eq!(app.current_dir, dir_one);

        app.next_tab().unwrap();
        assert_eq!(app.current_dir, dir_two);
    }

    #[derive(Default)]
    struct RecordingOpener {
        opened_paths: std::cell::RefCell<Vec<PathBuf>>,
    }

    impl EntryOpener for RecordingOpener {
        fn open(&self, path: &Path) -> AppResult<()> {
            self.opened_paths.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }
}
