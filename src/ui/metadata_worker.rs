use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::core::{
    EntryMetadata, FetchPriority, FetchQueue, MetadataRequest, RequestId, entry_metadata,
};
use crate::error::AppResult;

pub struct MetadataWorker {
    request_tx: Sender<MetadataCommand>,
    result_rx: Receiver<MetadataResult>,
}

pub struct MetadataResult {
    pub request_id: RequestId,
    pub path: PathBuf,
    pub metadata: AppResult<EntryMetadata>,
}

enum MetadataCommand {
    Enqueue(MetadataRequest),
    Cancel(RequestId),
}

impl MetadataWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<MetadataCommand>();
        let (result_tx, result_rx) = mpsc::channel::<MetadataResult>();

        thread::spawn(move || {
            let mut queue = FetchQueue::new(1);
            for command in request_rx {
                match command {
                    MetadataCommand::Enqueue(request) => {
                        queue.enqueue(request);
                    }
                    MetadataCommand::Cancel(request_id) => {
                        queue.cancel(request_id);
                    }
                }
                while let Some(request) = queue.start_next() {
                    let metadata = entry_metadata(&request.path);
                    let _ = result_tx.send(MetadataResult {
                        request_id: request.request_id,
                        path: request.path,
                        metadata,
                    });
                    queue.complete(request.request_id);
                }
            }
        });

        Self {
            request_tx,
            result_rx,
        }
    }

    pub fn request(&self, request_id: RequestId, path: PathBuf, priority: FetchPriority) {
        let _ = self.request_tx.send(MetadataCommand::Enqueue(MetadataRequest {
            request_id,
            path,
            priority,
        }));
    }

    pub fn cancel(&self, request_id: RequestId) {
        let _ = self.request_tx.send(MetadataCommand::Cancel(request_id));
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

        let request_id = RequestId::new().next();
        let worker = MetadataWorker::new();
        worker.request(request_id, file_path.clone(), FetchPriority::High);

        let result = worker
            .result_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        assert_eq!(result.request_id, request_id);
        assert_eq!(result.path, file_path);
        assert!(result.metadata.is_ok());
    }
}
