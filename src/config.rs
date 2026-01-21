use std::path::{Path, PathBuf};

use crate::core::ColorThemeId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub default_theme: Option<ColorThemeId>,
    pub allow_shell: bool,
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
            _ => continue,
        }
    }
    Config {
        default_theme,
        allow_shell,
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

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join(Path::new("oxide").join("config.toml")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_reads_default_theme() {
        let config = parse_config("default_theme = \"Glacier Coast\"");

        assert_eq!(
            config.default_theme,
            Some(ColorThemeId::GlacierCoast)
        );
    }

    #[test]
    fn parse_config_reads_allow_shell() {
        let config = parse_config("allow_shell = true");

        assert!(config.allow_shell);
    }

    #[test]
    fn parse_string_value_strips_comments_and_quotes() {
        let value = parse_string_value(" \"Night Harbor\" # comment").unwrap();

        assert_eq!(value, "Night Harbor");
    }
}
