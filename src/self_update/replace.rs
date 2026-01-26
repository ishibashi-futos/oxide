use crate::self_update::error::SelfUpdateError;
use std::path::{Path, PathBuf};

pub fn backup_path_for(current_exe: &Path, version_tag: &str) -> PathBuf {
    let name = format!("ox-{}", version_tag);
    current_exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

pub fn replace_current_exe(
    downloaded: &Path,
    version_tag: &str,
) -> Result<PathBuf, SelfUpdateError> {
    let current_exe = std::env::current_exe()?;
    let backup = backup_path_for(&current_exe, version_tag);
    let temp = current_exe.with_extension("new");

    if temp.exists() {
        std::fs::remove_file(&temp)?;
    }

    let source = prepare_replacement_binary(downloaded)?;

    std::fs::copy(&source, &temp)?;

    #[cfg(unix)]
    {
        let current_perms = std::fs::metadata(&current_exe)?.permissions();
        std::fs::set_permissions(&temp, current_perms)?;
        if !backup.exists() {
            std::fs::copy(&current_exe, &backup)?;
        }
    }

    #[cfg(windows)]
    {
        if backup.exists() {
            std::fs::remove_file(&backup)?;
        }
        std::fs::rename(&current_exe, &backup)?;
    }

    std::fs::rename(&temp, &current_exe)?;
    Ok(backup)
}

pub fn prepare_replacement_binary(downloaded: &Path) -> Result<PathBuf, SelfUpdateError> {
    if is_zip_file(downloaded)? {
        extract_zip_binary(downloaded)
    } else {
        Ok(downloaded.to_path_buf())
    }
}

fn is_zip_file(path: &Path) -> Result<bool, SelfUpdateError> {
    let is_extension_zip = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
    if is_extension_zip {
        return Ok(true);
    }
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 4];
    let read = std::io::Read::read(&mut file, &mut header)?;
    if read < 4 {
        return Ok(false);
    }
    Ok(matches!(
        &header,
        b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08"
    ))
}

fn extract_zip_binary(path: &Path) -> Result<PathBuf, SelfUpdateError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| SelfUpdateError::Io(std::io::Error::other("time error")))?;
    let mut dir = std::env::temp_dir();
    let safe_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    dir.push(format!("ox-extract-{}-{}", stamp.as_millis(), safe_name));
    std::fs::create_dir_all(&dir)?;
    extract_zip_to(path, &dir)?;
    find_binary_in_dir(&dir)
        .ok_or_else(|| SelfUpdateError::MissingBinaryInArchive(path.display().to_string()))
}

fn extract_zip_to(path: &Path, dir: &Path) -> Result<(), SelfUpdateError> {
    let file = std::fs::File::open(path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| zip_error("zip open failed", err))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| zip_error("zip read failed", err))?;
        let Some(entry_path) = entry.enclosed_name() else {
            continue;
        };
        let outpath = dir.join(entry_path);
        if entry.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }
    Ok(())
}

fn zip_error(context: &'static str, err: zip::result::ZipError) -> SelfUpdateError {
    SelfUpdateError::Io(std::io::Error::other(format!("{context}: {err}")))
}

fn find_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let entries = std::fs::read_dir(&next).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_binary_name(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn is_binary_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    #[cfg(windows)]
    {
        name == "ox.exe"
    }
    #[cfg(not(windows))]
    {
        name == "ox"
    }
}

pub fn list_backups() -> Result<Vec<PathBuf>, SelfUpdateError> {
    let current_exe = std::env::current_exe()?;
    let dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("ox-") {
            backups.push(path);
        }
    }
    backups.sort();
    Ok(backups)
}

pub fn rollback_named(backup: &Path) -> Result<PathBuf, SelfUpdateError> {
    let current_exe = std::env::current_exe()?;
    std::fs::copy(backup, &current_exe)?;
    Ok(backup.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn prepare_replacement_binary_extracts_zip_without_extension() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("ox-download");
        let binary_name = expected_binary_name();
        let payload = b"hello";

        let file = std::fs::File::create(&zip_path).expect("zip file");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        writer
            .start_file(format!("bin/{binary_name}"), options)
            .expect("start file");
        writer.write_all(payload).expect("write");
        writer.finish().expect("finish");

        let extracted = prepare_replacement_binary(&zip_path).expect("extract");
        let name = extracted.file_name().and_then(|value| value.to_str());
        assert_eq!(name, Some(binary_name));
        let contents = std::fs::read(&extracted).expect("read");
        assert_eq!(contents, payload);
    }

    #[cfg(windows)]
    fn expected_binary_name() -> &'static str {
        "ox.exe"
    }

    #[cfg(not(windows))]
    fn expected_binary_name() -> &'static str {
        "ox"
    }
}
