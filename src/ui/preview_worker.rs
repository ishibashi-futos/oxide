use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::core::{
    PreviewContent, PreviewEvent, PreviewFailed, PreviewReady, PreviewRequest, load_preview,
};

pub struct PreviewWorker {
    request_tx: Sender<PreviewRequest>,
    result_rx: Receiver<PreviewEvent>,
}

impl PreviewWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PreviewRequest>();
        let (result_tx, result_rx) = mpsc::channel::<PreviewEvent>();

        thread::spawn(move || {
            for request in request_rx {
                let _ = result_tx.send(PreviewEvent::Loading { id: request.id });
                match load_preview(&request.path, request.max_bytes) {
                    Ok(content) => {
                        let ready = build_ready(request.id, content);
                        let _ = result_tx.send(PreviewEvent::Ready(ready));
                    }
                    Err(reason) => {
                        let failed = PreviewFailed {
                            id: request.id,
                            reason,
                        };
                        let _ = result_tx.send(PreviewEvent::Failed(failed));
                    }
                }
            }
        });

        Self {
            request_tx,
            result_rx,
        }
    }

    pub fn request(&self, request: PreviewRequest) {
        let _ = self.request_tx.send(request);
    }

    pub fn poll(&self) -> Option<PreviewEvent> {
        self.result_rx.try_recv().ok()
    }
}

fn build_ready(id: u64, content: PreviewContent) -> PreviewReady {
    PreviewReady {
        id,
        lines: content.lines,
        truncated: content.truncated,
        reason: content.reason,
        kind_flags: content.kind_flags,
    }
}
