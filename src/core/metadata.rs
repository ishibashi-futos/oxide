use std::path::Path;
use std::time::SystemTime;

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    pub size: u64,
    pub modified: SystemTime,
}

pub fn entry_metadata(path: &Path) -> AppResult<EntryMetadata> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(EntryMetadata {
        size: metadata.len(),
        modified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn entry_metadata_returns_size_and_modified() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let metadata = entry_metadata(&file_path).unwrap();

        assert_eq!(metadata.size, 5);
        assert!(metadata.modified >= UNIX_EPOCH);
    }
}
