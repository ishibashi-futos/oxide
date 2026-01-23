use std::path::{Path, PathBuf};
use std::thread;

use crate::config::config_root;

pub fn load_session_tabs() -> Vec<SessionTab> {
    if cfg!(test) && std::env::var_os("OX_TEST_ALLOW_CONFIG").is_none() {
        return Vec::new();
    }
    let Some(path) = session_path() else {
        return Vec::new();
    };
    load_session_tabs_from(&path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTab {
    pub tab_id: u64,
    pub path: PathBuf,
    pub theme_name: String,
}

pub fn save_session_async(tabs: Vec<SessionTab>) {
    let Some(path) = session_path() else {
        return;
    };
    let payload = build_session_payload(&tabs);
    thread::spawn(move || {
        if let Err(error) = write_session_atomic(&path, payload.as_bytes()) {
            eprintln!("session save failed: {error}");
        }
    });
}

fn session_path() -> Option<PathBuf> {
    config_root().map(|root| root.join("session.json"))
}

fn load_session_tabs_from(path: &Path) -> Vec<SessionTab> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_session_tabs(&content)
}

fn parse_session_tabs(content: &str) -> Vec<SessionTab> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let Some(tabs) = value.get("tabs").and_then(|tabs| tabs.as_array()) else {
        return Vec::new();
    };
    tabs.iter()
        .enumerate()
        .filter_map(|(index, tab)| {
            let path = tab.get("path").and_then(|path| path.as_str())?;
            let tab_id = tab
                .get("tab_id")
                .and_then(|id| id.as_u64())
                .unwrap_or(index as u64 + 1);
            let theme_name = tab
                .get("theme")
                .and_then(|theme| theme.as_str())
                .unwrap_or("")
                .to_string();
            Some(SessionTab {
                tab_id,
                path: PathBuf::from(path),
                theme_name,
            })
        })
        .collect()
}

fn build_session_payload(tabs: &[SessionTab]) -> String {
    let tabs_json = tabs
        .iter()
        .map(|tab| {
            serde_json::json!({
                "tab_id": tab.tab_id,
                "path": tab.path.to_string_lossy(),
                "theme": tab.theme_name,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "version": 1,
        "session_id": "local",
        "tabs": tabs_json,
    })
    .to_string()
}

fn write_session_atomic(path: &Path, payload: &[u8]) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)?;
    let tmp_path = path.with_file_name("session.json.tmp");
    std::fs::write(&tmp_path, payload)?;
    if let Err(error) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path).map_err(|_| error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_session_tabs_reads_theme_and_id() {
        let content = r#"{
  "version": 1,
  "session_id": "test",
  "tabs": [
    { "tab_id": 9, "path": "/one", "theme": "Night Harbor" },
    { "path": "/two" }
  ]
}"#;

        let tabs = parse_session_tabs(content);

        assert_eq!(
            tabs,
            vec![
                SessionTab {
                    tab_id: 9,
                    path: PathBuf::from("/one"),
                    theme_name: "Night Harbor".to_string(),
                },
                SessionTab {
                    tab_id: 2,
                    path: PathBuf::from("/two"),
                    theme_name: "".to_string(),
                },
            ]
        );
    }

    #[test]
    fn build_session_payload_includes_theme_name() {
        let tabs = vec![SessionTab {
            tab_id: 1,
            path: PathBuf::from("/one"),
            theme_name: "Glacier Coast".to_string(),
        }];

        let payload = build_session_payload(&tabs);

        let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        let tab = value
            .get("tabs")
            .and_then(|tabs| tabs.as_array())
            .and_then(|tabs| tabs.first())
            .expect("tab");
        assert_eq!(
            tab.get("theme").and_then(|v| v.as_str()),
            Some("Glacier Coast")
        );
    }
}
