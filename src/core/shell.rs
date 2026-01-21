use std::path::{Path, PathBuf};
use std::process::Command;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellExecutionError {
    SpawnFailed(String),
}

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
        let mut args = Vec::new();
        let mut buffer = String::new();
        let mut quote: Option<char> = None;
        for ch in trimmed.chars() {
            match quote {
                Some(active) => {
                    if ch == active {
                        quote = None;
                    } else {
                        buffer.push(ch);
                    }
                }
                None => match ch {
                    '\'' | '"' => {
                        quote = Some(ch);
                    }
                    ch if ch.is_whitespace() => {
                        if !buffer.is_empty() {
                            args.push(buffer.clone());
                            buffer.clear();
                        }
                    }
                    _ => buffer.push(ch),
                },
            }
        }
        if quote.is_some() {
            return Err(ShellCommandError::UnterminatedQuote);
        }
        if !buffer.is_empty() {
            args.push(buffer);
        }
        if args.is_empty() {
            return Err(ShellCommandError::MissingCommand);
        }
        Ok(args)
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

    pub fn execute(
        &self,
        request: &ShellCommandRequest,
    ) -> Result<ShellExecutionResult, ShellExecutionError> {
        let start = Instant::now();
        let timestamp_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let mut command = Command::new(self.allowed_shell.path());
        command
            .args(self.allowed_shell.args(&request.raw_command))
            .current_dir(&request.working_dir);
        if !self.allowed_shell.inherit_env() {
            command.env_clear();
            for (key, value) in self.safe_env.entries.iter() {
                command.env(key, value);
            }
        }
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
        let mut entries = Vec::new();
        let keys = std::env::var("OX_SAFE_ENV").unwrap_or_default();
        for key in keys.split(',') {
            let name = key.trim();
            if name.is_empty() {
                continue;
            }
            if let Ok(value) = std::env::var(name) {
                entries.push((name.to_string(), value));
            }
        }
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
    for arg in args {
        if arg.trim().is_empty() {
            continue;
        }
        let candidate = Path::new(arg);
        if !looks_like_path(candidate) {
            continue;
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
        if !normalized.starts_with(&base) {
            return Err(ShellCommandError::PathEscapesWorkingDir);
        }
    }
    Ok(())
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn looks_like_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let mut count = 0;
    for component in path.components() {
        count += 1;
        match component {
            std::path::Component::CurDir | std::path::Component::ParentDir => return true,
            _ => {}
        }
    }
    count > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_shell_operators() {
        let samples = ["echo a && b", "echo a; b", "echo a | b", "echo $(a)", "`echo a`"];
        for input in samples {
            let result = ShellCommandParser::sanitize_args(input);
            assert_eq!(result, Err(ShellCommandError::ForbiddenOperator));
        }
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
}
