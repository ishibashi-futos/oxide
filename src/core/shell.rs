use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(test)]
use std::time::{Instant, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommandRequest {
    pub working_dir: PathBuf,
    pub raw_command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellExecutionResult {
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
    pub timestamp_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellCommandError {
    MissingCommand,
    ForbiddenOperator,
    UnterminatedQuote,
    PathEscapesWorkingDir,
}

impl std::fmt::Display for ShellCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            ShellCommandError::MissingCommand => "shell: missing command",
            ShellCommandError::ForbiddenOperator => "連結演算子は禁止",
            ShellCommandError::UnterminatedQuote => "shell: unterminated quote",
            ShellCommandError::PathEscapesWorkingDir => "shell: path escapes working dir",
        };
        write!(f, "{message}")
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellExecutionError {
    SpawnFailed(String),
}

#[cfg(test)]
impl std::fmt::Display for ShellExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellExecutionError::SpawnFailed(message) => write!(f, "shell: {message}"),
        }
    }
}

pub struct ShellCommandParser;

impl ShellCommandParser {
    pub fn sanitize_args(input: &str) -> Result<(), ShellCommandError> {
        let forbidden = ["&&", ";", "|", "$(", "`"];
        if forbidden.iter().any(|pattern| input.contains(pattern)) {
            return Err(ShellCommandError::ForbiddenOperator);
        }
        Ok(())
    }

    pub fn parse_args(input: &str) -> Result<Vec<String>, ShellCommandError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ShellCommandError::MissingCommand);
        }
        let mut state = trimmed.chars().fold(ParseState::new(), |mut state, ch| {
            match state.quote {
                Some(active) => {
                    if ch == active {
                        state.quote = None;
                    } else {
                        state.buffer.push(ch);
                    }
                }
                None => match ch {
                    '\'' | '"' => {
                        state.quote = Some(ch);
                    }
                    ch if ch.is_whitespace() => {
                        state.push_buffer();
                    }
                    _ => state.buffer.push(ch),
                },
            }
            state
        });
        if state.quote.is_some() {
            return Err(ShellCommandError::UnterminatedQuote);
        }
        state.push_buffer();
        if state.args.is_empty() {
            return Err(ShellCommandError::MissingCommand);
        }
        Ok(state.args)
    }
}

#[derive(Debug)]
struct ParseState {
    args: Vec<String>,
    buffer: String,
    quote: Option<char>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            args: Vec::new(),
            buffer: String::new(),
            quote: None,
        }
    }

    fn push_buffer(&mut self) {
        if !self.buffer.is_empty() {
            self.args.push(std::mem::take(&mut self.buffer));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPermission {
    allow_env: bool,
    allow_config: bool,
}

impl ShellPermission {
    pub fn new(allow_env: bool, allow_config: bool) -> Self {
        Self {
            allow_env,
            allow_config,
        }
    }

    pub fn from_env(allow_config: bool) -> Self {
        let allow_env = std::env::var("OX_ALLOW_SHELL")
            .ok()
            .map(|value| parse_bool(&value))
            .unwrap_or(false);
        Self::new(allow_env, allow_config)
    }

    pub fn is_allowed(&self) -> bool {
        self.allow_env || self.allow_config
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedShell {
    Default,
}

impl AllowedShell {
    pub fn path(self) -> &'static str {
        match self {
            AllowedShell::Default => default_shell_path(),
        }
    }

    pub fn args(self, command: &str) -> Vec<String> {
        match self {
            AllowedShell::Default => default_shell_args(command),
        }
    }

    pub fn inherit_env(self) -> bool {
        match self {
            AllowedShell::Default => default_shell_inherit_env(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShellExecutionGuard {
    allowed_shell: AllowedShell,
    safe_env: SafeEnv,
}

impl ShellExecutionGuard {
    pub fn new() -> Self {
        Self {
            allowed_shell: AllowedShell::Default,
            safe_env: SafeEnv::from_env(),
        }
    }

    #[cfg(test)]
    pub fn execute(
        &self,
        request: &ShellCommandRequest,
    ) -> Result<ShellExecutionResult, ShellExecutionError> {
        let start = Instant::now();
        let timestamp_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let mut command = self.build_command(request);
        let output = command
            .output()
            .map_err(|error| ShellExecutionError::SpawnFailed(error.to_string()))?;
        Ok(ShellExecutionResult {
            status_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: start.elapsed().as_millis(),
            timestamp_ms,
        })
    }

    pub(crate) fn build_command(&self, request: &ShellCommandRequest) -> Command {
        let mut command = Command::new(self.allowed_shell.path());
        command
            .args(self.allowed_shell.args(&request.raw_command))
            .current_dir(&request.working_dir);
        if !self.allowed_shell.inherit_env() {
            command.env_clear();
            self.safe_env
                .entries
                .iter()
                .for_each(|(key, value)| {
                    command.env(key, value);
                });
        }
        command
    }
}

#[cfg(target_os = "macos")]
fn default_shell_path() -> &'static str {
    "/bin/zsh"
}

#[cfg(target_os = "macos")]
fn default_shell_args(command: &str) -> Vec<String> {
    vec!["-lc".to_string(), command.to_string()]
}

#[cfg(target_os = "macos")]
fn default_shell_inherit_env() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
fn default_shell_path() -> &'static str {
    "sh"
}

#[cfg(not(target_os = "macos"))]
fn default_shell_args(command: &str) -> Vec<String> {
    vec!["-c".to_string(), command.to_string()]
}

#[cfg(not(target_os = "macos"))]
fn default_shell_inherit_env() -> bool {
    false
}

impl ShellCommandRequest {
    pub fn new(working_dir: PathBuf, raw_command: &str) -> Result<Self, ShellCommandError> {
        let raw_command = raw_command.trim();
        ShellCommandParser::sanitize_args(raw_command)?;
        let args = ShellCommandParser::parse_args(raw_command)?;
        ensure_args_within_working_dir(&working_dir, &args)?;
        Ok(Self {
            working_dir,
            raw_command: raw_command.to_string(),
            args,
        })
    }
}

#[derive(Debug, Clone)]
struct SafeEnv {
    entries: Vec<(String, String)>,
}

impl SafeEnv {
    fn from_env() -> Self {
        let keys = std::env::var("OX_SAFE_ENV").unwrap_or_default();
        let entries = keys
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| (name.to_string(), value))
            })
            .collect();
        Self { entries }
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn ensure_args_within_working_dir(
    working_dir: &Path,
    args: &[String],
) -> Result<(), ShellCommandError> {
    let base = working_dir
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(working_dir));
    let base = comparable_path(&base);
    #[cfg(windows)]
    let base_str = comparable_path_string(&base);
    args.iter().try_for_each(|arg| {
        if arg.trim().is_empty() {
            return Ok(());
        }
        let candidate = Path::new(arg);
        if !looks_like_path(candidate) {
            return Ok(());
        }
        let full = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            working_dir.join(candidate)
        };
        let normalized = if full.exists() {
            full.canonicalize().unwrap_or_else(|_| normalize_path(&full))
        } else {
            normalize_path(&full)
        };
        let normalized = comparable_path(&normalized);
        #[cfg(windows)]
        {
            let normalized_str = comparable_path_string(&normalized);
            if !is_within_dir_string(&base_str, &normalized_str) {
                return Err(ShellCommandError::PathEscapesWorkingDir);
            }
            return Ok(());
        }
        #[cfg(not(windows))]
        if !normalized.starts_with(&base) {
            return Err(ShellCommandError::PathEscapesWorkingDir);
        }
        Ok(())
    })
}

fn comparable_path(path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    #[cfg(windows)]
    {
        strip_verbatim_prefix(&normalized)
    }
    #[cfg(not(windows))]
    {
        normalized
    }
}

#[cfg(windows)]
fn comparable_path_string(path: &Path) -> String {
    let normalized = normalize_path(path);
    let normalized = strip_verbatim_prefix(&normalized);
    let value = normalized.to_string_lossy().replace('/', "\\");
    value.to_ascii_lowercase()
}

#[cfg(windows)]
fn is_within_dir_string(base: &str, candidate: &str) -> bool {
    if candidate == base {
        return true;
    }
    let mut prefix = String::with_capacity(base.len() + 1);
    prefix.push_str(base);
    if !base.ends_with('\\') {
        prefix.push('\\');
    }
    candidate.starts_with(&prefix)
}

#[cfg(windows)]
fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let value = path.as_os_str().to_string_lossy();
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().fold(PathBuf::new(), |mut normalized, component| {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
        normalized
    })
}

fn looks_like_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let (count, seen_dot) =
        path.components().fold((0usize, false), |(count, seen_dot), component| {
            let count = count + 1;
            let seen_dot = seen_dot
                || matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                );
            (count, seen_dot)
        });
    seen_dot || count > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_shell_operators() {
        let samples = ["echo a && b", "echo a; b", "echo a | b", "echo $(a)", "`echo a`"];
        samples.iter().for_each(|input| {
            let result = ShellCommandParser::sanitize_args(input);
            assert_eq!(result, Err(ShellCommandError::ForbiddenOperator));
        });
    }

    #[test]
    fn parse_args_supports_quotes() {
        let args = ShellCommandParser::parse_args("echo \"foo bar\"").unwrap();
        assert_eq!(args, vec!["echo".to_string(), "foo bar".to_string()]);

        let args = ShellCommandParser::parse_args("\"echo foo\"").unwrap();
        assert_eq!(args, vec!["echo foo".to_string()]);
    }

    #[test]
    fn parse_args_rejects_unterminated_quote() {
        let result = ShellCommandParser::parse_args("echo \"foo");
        assert_eq!(result, Err(ShellCommandError::UnterminatedQuote));
    }

    #[test]
    fn request_rejects_path_escape() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = ShellCommandRequest::new(temp_dir.path().to_path_buf(), "ls ..");
        assert_eq!(result, Err(ShellCommandError::PathEscapesWorkingDir));
    }

    #[test]
    fn execute_shell_command_returns_stdout() {
        let temp_dir = tempfile::tempdir().unwrap();
        let request = ShellCommandRequest::new(temp_dir.path().to_path_buf(), "echo hi").unwrap();
        let guard = ShellExecutionGuard::new();

        let result = guard.execute(&request).unwrap();

        assert_eq!(result.status_code, Some(0));
        assert!(result.stdout.trim().contains("hi"));
    }

    #[cfg(windows)]
    #[test]
    fn comparable_path_strips_verbatim_prefix() {
        let path = PathBuf::from(r"\\?\C:\Temp\foo");
        let base = PathBuf::from(r"C:\Temp");
        let normalized = comparable_path(&path);
        assert!(normalized.starts_with(&base));
    }
}
