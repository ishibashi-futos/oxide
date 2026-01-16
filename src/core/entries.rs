use std::path::Path;

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
}

pub fn list_entries(path: &Path, include_hidden: bool) -> AppResult<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !include_hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type()?.is_dir();
        entries.push(Entry { name, is_dir });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_entries_returns_file_names() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_b = temp_dir.path().join("beta.txt");
        let file_a = temp_dir.path().join("alpha.txt");
        fs::write(&file_b, "b").unwrap();
        fs::write(&file_a, "a").unwrap();

        let entries = list_entries(temp_dir.path(), true).unwrap();

        assert_eq!(
            entries,
            vec![
                Entry {
                    name: "alpha.txt".to_string(),
                    is_dir: false,
                },
                Entry {
                    name: "beta.txt".to_string(),
                    is_dir: false,
                }
            ]
        );
    }

    #[test]
    fn list_entries_marks_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir = temp_dir.path().join("child");
        fs::create_dir(&dir).unwrap();

        let entries = list_entries(temp_dir.path(), true).unwrap();

        assert_eq!(
            entries,
            vec![Entry {
                name: "child".to_string(),
                is_dir: true,
            }]
        );
    }

    #[test]
    fn list_entries_excludes_hidden_by_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join(".secret");
        fs::write(&file, "hidden").unwrap();

        let entries = list_entries(temp_dir.path(), false).unwrap();

        assert!(entries.is_empty());
    }

    #[test]
    fn list_entries_includes_hidden_when_enabled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join(".secret");
        fs::write(&file, "hidden").unwrap();

        let entries = list_entries(temp_dir.path(), true).unwrap();

        assert_eq!(
            entries,
            vec![Entry {
                name: ".secret".to_string(),
                is_dir: false,
            }]
        );
    }
}
