use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewContent {
    pub lines: Vec<String>,
    pub truncated: bool,
    pub reason: Option<String>,
    pub kind_flags: Vec<LineKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRequest {
    pub id: u64,
    pub path: PathBuf,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewReady {
    pub id: u64,
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub truncated: bool,
    pub reason: Option<String>,
    pub kind_flags: Vec<LineKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewFailed {
    pub id: u64,
    pub reason: PreviewError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewEvent {
    Loading { id: u64 },
    Ready(PreviewReady),
    Failed(PreviewFailed),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    TooLarge,
    BinaryFile,
    IoError(String),
    PermissionDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Heading,
    ListItem,
    CodeFence,
    Normal,
}

pub fn load_preview(path: &Path, max_bytes: usize) -> Result<PreviewContent, PreviewError> {
    let metadata = std::fs::metadata(path).map_err(map_io_error)?;
    if metadata.len() as usize > max_bytes {
        return Err(PreviewError::TooLarge);
    }

    let mut file = File::open(path).map_err(map_io_error)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(map_io_error)?;
    if buffer.len() > max_bytes {
        return Err(PreviewError::TooLarge);
    }
    if buffer.contains(&0) {
        return Err(PreviewError::BinaryFile);
    }

    let (text, reason) = match String::from_utf8(buffer) {
        Ok(text) => (Cow::Owned(text), None),
        Err(error) => {
            let bytes = error.into_bytes();
            let lossy = String::from_utf8_lossy(&bytes).into_owned();
            (
                Cow::Owned(lossy),
                Some("非UTF-8のため簡易モード".to_string()),
            )
        }
    };

    Ok(build_preview_content(&text, reason))
}

fn build_preview_content(text: &str, reason: Option<String>) -> PreviewContent {
    const MAX_LINES: usize = 40;
    const MAX_LINE_WIDTH: usize = 120;
    let mut lines = Vec::new();
    let mut kind_flags = Vec::new();
    for line in text.lines().take(MAX_LINES) {
        let normalized = normalize_line(line, MAX_LINE_WIDTH);
        kind_flags.push(detect_line_kind(&normalized));
        lines.push(normalized);
    }
    let truncated = text.lines().count() > MAX_LINES;
    PreviewContent {
        lines,
        truncated,
        reason,
        kind_flags,
    }
}

fn normalize_line(line: &str, max_width: usize) -> String {
    let count = line.chars().count();
    if count <= max_width {
        return line.to_string();
    }
    let trimmed: String = line.chars().take(max_width.saturating_sub(1)).collect();
    format!("{trimmed}…")
}

fn detect_line_kind(line: &str) -> LineKind {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return LineKind::Heading;
    }
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return LineKind::ListItem;
    }
    if trimmed.starts_with("```") {
        return LineKind::CodeFence;
    }
    LineKind::Normal
}

fn map_io_error(error: std::io::Error) -> PreviewError {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::PermissionDenied => PreviewError::PermissionDenied,
        _ => PreviewError::IoError(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_preview_reads_first_40_lines() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.txt");
        let mut file = File::create(&file_path).unwrap();
        for index in 0..45 {
            writeln!(file, "line-{index}").unwrap();
        }

        let preview = load_preview(&file_path, 1024 * 1024).unwrap();

        assert_eq!(preview.lines.len(), 40);
        assert!(preview.truncated);
        assert_eq!(preview.reason, None);
    }

    #[test]
    fn load_preview_marks_markdown_kinds() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.md");
        std::fs::write(&file_path, "# Title\n- item\n```\ncode\nplain\n").unwrap();

        let preview = load_preview(&file_path, 1024 * 1024).unwrap();

        assert_eq!(
            preview.kind_flags,
            vec![
                LineKind::Heading,
                LineKind::ListItem,
                LineKind::CodeFence,
                LineKind::Normal,
                LineKind::Normal,
            ]
        );
    }

    #[test]
    fn load_preview_returns_reason_for_non_utf8() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.bin");
        std::fs::write(&file_path, vec![0xF0, 0x28, 0x8C, 0x28]).unwrap();

        let preview = load_preview(&file_path, 1024 * 1024).unwrap();

        assert_eq!(preview.reason, Some("非UTF-8のため簡易モード".to_string()));
    }

    #[test]
    fn load_preview_fails_when_too_large() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("big.txt");
        let content = vec![b'a'; 1024];
        std::fs::write(&file_path, content).unwrap();

        let result = load_preview(&file_path, 10);

        assert_eq!(result.unwrap_err(), PreviewError::TooLarge);
    }
}
