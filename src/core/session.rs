use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread;

use crate::config::config_root;
use uuid::Uuid;

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
pub enum SessionEvent {
    SaveFailed,
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
    let session_id = generate_session_id();
    let payload = build_session_payload(&tabs, &session_id);
    let history_path = session_history_path(&session_id);
    thread::spawn(move || {
        let mut notified = false;
        if write_session_atomic(&path, payload.as_bytes()).is_err() && !notified {
            notify_session_event(SessionEvent::SaveFailed);
            notified = true;
        }
        if let Some(path) = history_path {
            if write_session_atomic(&path, payload.as_bytes()).is_err() {
                if !notified {
                    notify_session_event(SessionEvent::SaveFailed);
                }
            } else if let Some(dir) = path.parent()
                && prune_session_history(dir, 50).is_err()
                && !notified
            {
                notify_session_event(SessionEvent::SaveFailed);
            }
        }
    });
}

pub fn poll_session_events() -> Vec<SessionEvent> {
    let bus = session_event_bus();
    let receiver = bus.receiver.lock().expect("session event lock");
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    events
}

struct SessionEventBus {
    sender: mpsc::Sender<SessionEvent>,
    receiver: Mutex<mpsc::Receiver<SessionEvent>>,
}

fn session_event_bus() -> &'static SessionEventBus {
    static BUS: OnceLock<SessionEventBus> = OnceLock::new();
    BUS.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        SessionEventBus {
            sender,
            receiver: Mutex::new(receiver),
        }
    })
}

fn notify_session_event(event: SessionEvent) {
    let _ = session_event_bus().sender.send(event);
}

fn session_path() -> Option<PathBuf> {
    config_root().map(|root| root.join("session.json"))
}

fn session_history_path(session_id: &str) -> Option<PathBuf> {
    config_root().map(|root| session_history_dir(&root).join(format!("{session_id}.json")))
}

fn session_history_dir(root: &Path) -> PathBuf {
    root.join("sessions")
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

fn build_session_payload(tabs: &[SessionTab], session_id: &str) -> String {
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
        "session_id": session_id,
        "tabs": tabs_json,
    })
    .to_string()
}

fn generate_session_id() -> String {
    Uuid::now_v7().to_string()
}

fn write_session_atomic(path: &Path, payload: &[u8]) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, payload)?;
    if let Err(error) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path).map_err(|_| error)?;
    }
    Ok(())
}

fn prune_session_history(dir: &Path, keep: usize) -> std::io::Result<()> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = file_name.strip_suffix(".json") else {
            continue;
        };
        if Uuid::parse_str(stem).is_err() {
            continue;
        }
        entries.push((file_name.to_string(), path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    if entries.len() <= keep {
        return Ok(());
    }
    let remove_count = entries.len() - keep;
    for (_, path) in entries.into_iter().take(remove_count) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
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

        let payload = build_session_payload(&tabs, "test-session");

        let value: serde_json::Value = serde_json::from_str(&payload).expect("json");
        assert_eq!(
            value.get("session_id").and_then(|v| v.as_str()),
            Some("test-session")
        );
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

    #[test]
    fn prune_session_history_keeps_latest_50() {
        let dir = tempdir().expect("tempdir");
        for i in 1u128..=55 {
            let name = format!("{}.json", Uuid::from_u128(i));
            let path = dir.path().join(name);
            std::fs::write(path, "{}").expect("write");
        }

        prune_session_history(dir.path(), 50).expect("prune");

        let mut names = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names.len(), 50);
        assert!(!names.contains(&format!("{}.json", Uuid::from_u128(1))));
        assert!(!names.contains(&format!("{}.json", Uuid::from_u128(5))));
        assert!(names.contains(&format!("{}.json", Uuid::from_u128(6))));
        assert!(names.contains(&format!("{}.json", Uuid::from_u128(55))));
    }

    #[test]
    fn save_session_emits_event_on_failure() {
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
        let _ = poll_session_events();

        save_session_async(vec![SessionTab {
            tab_id: 1,
            path: PathBuf::from("/"),
            theme_name: String::new(),
        }]);

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            events = poll_session_events();
            if !events.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

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

        assert!(
            events
                .iter()
                .any(|event| matches!(event, SessionEvent::SaveFailed))
        );
    }
}
