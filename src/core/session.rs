use std::path::{Path, PathBuf};

use crate::config::config_root;

pub fn load_session_paths() -> Vec<PathBuf> {
    let Some(path) = session_path() else {
        return Vec::new();
    };
    load_session_paths_from(&path)
}

fn session_path() -> Option<PathBuf> {
    config_root().map(|root| root.join("session.json"))
}

fn load_session_paths_from(path: &Path) -> Vec<PathBuf> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_session_paths(&content)
}

fn parse_session_paths(content: &str) -> Vec<PathBuf> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let Some(tabs) = value.get("tabs").and_then(|tabs| tabs.as_array()) else {
        return Vec::new();
    };
    tabs.iter()
        .filter_map(|tab| tab.get("path").and_then(|path| path.as_str()))
        .map(PathBuf::from)
        .collect()
}

pub fn restore_start_dir(default: PathBuf) -> PathBuf {
    let paths = load_session_paths();
    choose_restore_dir(&paths, default)
}

fn choose_restore_dir(paths: &[PathBuf], default: PathBuf) -> PathBuf {
    for path in paths {
        if path.is_dir() {
            return path.clone();
        }
    }
    default
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_session_paths_reads_tab_paths() {
        let content = r#"{
  "version": 1,
  "session_id": "test",
  "tabs": [
    { "tab_id": 1, "path": "/one" },
    { "tab_id": 2, "path": "/two" }
  ]
}"#;

        let paths = parse_session_paths(content);

        assert_eq!(paths, vec![PathBuf::from("/one"), PathBuf::from("/two")]);
    }

    #[test]
    fn load_session_paths_from_reads_file() {
        let dir = tempdir().expect("tempdir");
        let session_path = dir.path().join("session.json");
        std::fs::write(
            &session_path,
            r#"{
  "version": 1,
  "session_id": "test",
  "tabs": [
    { "tab_id": 1, "path": "alpha" }
  ]
}"#,
        )
        .expect("write session.json");

        let paths = load_session_paths_from(&session_path);

        assert_eq!(paths, vec![PathBuf::from("alpha")]);
    }

    #[test]
    fn choose_restore_dir_picks_first_existing_directory() {
        let dir = tempdir().expect("tempdir");
        let valid = dir.path().join("restore");
        std::fs::create_dir_all(&valid).expect("create dir");
        let paths = vec![PathBuf::from("/missing"), valid.clone()];

        let restored = choose_restore_dir(&paths, PathBuf::from("/fallback"));

        assert_eq!(restored, valid);
    }

    #[test]
    fn restore_start_dir_uses_default_when_session_missing() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var_os("OX_CONFIG_HOME");
        let temp_dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var("OX_CONFIG_HOME", temp_dir.path());
        }

        let restored = restore_start_dir(PathBuf::from("/fallback"));

        match previous {
            Some(value) => unsafe {
                std::env::set_var("OX_CONFIG_HOME", value);
            },
            None => unsafe {
                std::env::remove_var("OX_CONFIG_HOME");
            },
        }

        assert_eq!(restored, PathBuf::from("/fallback"));
    }
}
