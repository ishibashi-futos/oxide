use std::path::Path;

use crate::error::AppResult;

pub fn list_entries(path: &Path) -> AppResult<Vec<String>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(name);
    }
    entries.sort();
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

        let entries = list_entries(temp_dir.path()).unwrap();

        assert_eq!(entries, vec!["alpha.txt".to_string(), "beta.txt".to_string()]);
    }
}
