use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::ColorThemeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigEvent {
    ConfigRootUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub default_theme: Option<ColorThemeId>,
    pub allow_shell: bool,
    pub allow_opener: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_theme: None,
            allow_shell: false,
            allow_opener: default_allow_opener(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        parse_config(&content)
    }
}

fn parse_config(content: &str) -> Config {
    let mut default_theme = None;
    let mut allow_shell = false;
    let mut allow_opener = default_allow_opener();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        match key.trim() {
            "default_theme" => {
                if let Some(name) = parse_string_value(value) {
                    default_theme = ColorThemeId::from_name(&name);
                }
            }
            "allow_shell" => {
                allow_shell = parse_bool_value(value).unwrap_or(false);
            }
            "allow_opener" => {
                allow_opener = parse_bool_value(value).unwrap_or(true);
            }
            _ => continue,
        }
    }
    Config {
        default_theme,
        allow_shell,
        allow_opener,
    }
}

fn parse_string_value(value: &str) -> Option<String> {
    let mut raw = value.trim();
    if let Some(index) = raw.find('#') {
        raw = raw[..index].trim();
    }
    if raw.is_empty() {
        return None;
    }
    let unquoted = if (raw.starts_with('"') && raw.ends_with('"'))
        || (raw.starts_with('\'') && raw.ends_with('\''))
    {
        &raw[1..raw.len().saturating_sub(1)]
    } else {
        raw
    };
    let normalized = unquoted.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn parse_bool_value(value: &str) -> Option<bool> {
    let raw = parse_string_value(value)?;
    let normalized = raw.to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn default_allow_opener() -> bool {
    if cfg!(target_os = "linux") {
        return false;
    }
    true
}

fn config_path() -> Option<PathBuf> {
    config_root().map(|root| root.join("config.toml"))
}

pub fn config_root() -> Option<PathBuf> {
    if cfg!(test) && std::env::var_os("OX_TEST_ALLOW_CONFIG").is_none() {
        return None;
    }
    let base = std::env::var_os("OX_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let root = base.join(Path::new("oxide"));
    if std::fs::create_dir_all(&root).is_err() || !is_writable_dir(&root) {
        notify_config_event(ConfigEvent::ConfigRootUnavailable);
        return None;
    }
    Some(root)
}

pub fn poll_config_events() -> Vec<ConfigEvent> {
    let bus = config_event_bus();
    let receiver = bus.receiver.lock().expect("config event lock");
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}

struct ConfigEventBus {
    sender: mpsc::Sender<ConfigEvent>,
    receiver: Mutex<mpsc::Receiver<ConfigEvent>>,
}

fn config_event_bus() -> &'static ConfigEventBus {
    static BUS: OnceLock<ConfigEventBus> = OnceLock::new();
    BUS.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        ConfigEventBus {
            sender,
            receiver: Mutex::new(receiver),
        }
    })
}

fn notify_config_event(event: ConfigEvent) {
    let _ = config_event_bus().sender.send(event);
}

fn is_writable_dir(path: &Path) -> bool {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let probe = path.join(format!(".ox-write-test-{pid}-{nanos}"));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_config_reads_default_theme() {
        let config = parse_config("default_theme = \"Glacier Coast\"");

        assert_eq!(config.default_theme, Some(ColorThemeId::GlacierCoast));
    }

    #[test]
    fn parse_config_reads_allow_shell() {
        let config = parse_config("allow_shell = true");

        assert!(config.allow_shell);
    }

    #[test]
    fn parse_config_reads_allow_opener() {
        let config = parse_config("allow_opener = false");

        assert!(!config.allow_opener);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn default_allow_opener_is_false_on_linux() {
        assert!(!default_allow_opener());
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn default_allow_opener_is_true_on_non_linux() {
        assert!(default_allow_opener());
    }

    #[test]
    fn parse_string_value_strips_comments_and_quotes() {
        let value = parse_string_value(" \"Night Harbor\" # comment").unwrap();

        assert_eq!(value, "Night Harbor");
    }

    #[test]
    fn config_root_uses_ox_config_home() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir");
        let base = temp.path().to_path_buf();
        let prev_config = std::env::var_os("OX_CONFIG_HOME");
        let prev_allow = std::env::var_os("OX_TEST_ALLOW_CONFIG");

        unsafe {
            std::env::set_var("OX_CONFIG_HOME", &base);
            std::env::set_var("OX_TEST_ALLOW_CONFIG", "1");
        }

        let root = config_root().expect("config root");

        unsafe {
            match prev_config {
                Some(value) => std::env::set_var("OX_CONFIG_HOME", value),
                None => std::env::remove_var("OX_CONFIG_HOME"),
            }
            match prev_allow {
                Some(value) => std::env::set_var("OX_TEST_ALLOW_CONFIG", value),
                None => std::env::remove_var("OX_TEST_ALLOW_CONFIG"),
            }
        }

        assert_eq!(root, base.join("oxide"));
    }

    #[test]
    fn config_root_emits_event_when_unwritable() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = tempdir().expect("tempdir");
        let file_path = temp.path().join("config-file");
        std::fs::write(&file_path, b"nope").expect("write");

        let prev_config = std::env::var_os("OX_CONFIG_HOME");
        let prev_allow = std::env::var_os("OX_TEST_ALLOW_CONFIG");

        unsafe {
            std::env::set_var("OX_CONFIG_HOME", &file_path);
            std::env::set_var("OX_TEST_ALLOW_CONFIG", "1");
        }
        let _ = poll_config_events();

        let root = config_root();
        let events = poll_config_events();

        unsafe {
            match prev_config {
                Some(value) => std::env::set_var("OX_CONFIG_HOME", value),
                None => std::env::remove_var("OX_CONFIG_HOME"),
            }
            match prev_allow {
                Some(value) => std::env::set_var("OX_TEST_ALLOW_CONFIG", value),
                None => std::env::remove_var("OX_TEST_ALLOW_CONFIG"),
            }
        }

        assert!(root.is_none());
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ConfigEvent::ConfigRootUnavailable))
        );
    }
}
