use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::core::{EntryMetadata, entry_metadata};

pub struct MetadataWorker {
    request_tx: Sender<PathBuf>,
    result_rx: Receiver<MetadataResult>,
}

pub struct MetadataResult {
    pub path: PathBuf,
    pub metadata: Option<EntryMetadata>,
}

impl MetadataWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PathBuf>();
        let (result_tx, result_rx) = mpsc::channel::<MetadataResult>();

        thread::spawn(move || {
            for path in request_rx {
                let metadata = entry_metadata(&path).ok();
                let _ = result_tx.send(MetadataResult { path, metadata });
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

    pub fn poll(&self) -> Option<MetadataResult> {
        self.result_rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn metadata_worker_returns_metadata_for_requested_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let worker = MetadataWorker::new();
        worker.request(file_path.clone());

        let result = worker
            .result_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        assert_eq!(result.path, file_path);
        assert!(result.metadata.is_some());
    }
}
