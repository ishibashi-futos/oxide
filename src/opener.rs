use std::path::Path;
use std::process::Command;

use crate::app::EntryOpener;
use crate::error::AppResult;

pub struct PlatformOpener;

impl EntryOpener for PlatformOpener {
    fn open(&self, path: &Path) -> AppResult<()> {
        let status = open_command(path)?.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("open command failed: {status}"),
            )
            .into())
        }
    }
}

#[cfg(target_os = "macos")]
fn open_command(path: &Path) -> AppResult<Command> {
    let mut command = Command::new("open");
    command.arg(path);
    Ok(command)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_command(path: &Path) -> AppResult<Command> {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    Ok(command)
}

#[cfg(target_os = "windows")]
fn open_command(path: &Path) -> AppResult<Command> {
    let mut command = Command::new("cmd");
    command.arg("/C").arg("start").arg("").arg(path);
    Ok(command)
}
