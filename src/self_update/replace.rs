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

    std::fs::copy(downloaded, &temp)?;

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
