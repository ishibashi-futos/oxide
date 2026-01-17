use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub fn current_branch(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let mut branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        return None;
    }
    if branch == "HEAD" {
        branch = "HEAD(Detached)".to_string();
    }
    Some(branch)
}

pub struct GitWorker {
    request_tx: Sender<PathBuf>,
    result_rx: Receiver<GitResult>,
}

pub struct GitResult {
    pub path: PathBuf,
    pub branch: Option<String>,
}

impl GitWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PathBuf>();
        let (result_tx, result_rx) = mpsc::channel::<GitResult>();

        thread::spawn(move || {
            for path in request_rx {
                let branch = current_branch(&path);
                let _ = result_tx.send(GitResult { path, branch });
            }
        });

        Self {
            request_tx,
            result_rx,
        }
    }

    pub fn request(&self, path: PathBuf) {
        let _ = self.request_tx.send(path);
    }

    pub fn poll(&self) -> Option<GitResult> {
        self.result_rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn current_branch_returns_none_for_non_git_dir() {
        let temp_dir = tempfile::tempdir().unwrap();

        let branch = current_branch(temp_dir.path());

        assert!(branch.is_none());
    }

    #[test]
    fn git_worker_returns_none_for_non_git_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let worker = GitWorker::new();
        worker.request(temp_dir.path().to_path_buf());

        let result = worker
            .result_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        assert_eq!(result.path, temp_dir.path());
        assert!(result.branch.is_none());
    }
}
