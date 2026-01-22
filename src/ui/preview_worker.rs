use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::core::{
    PreviewContent, PreviewError, PreviewEvent, PreviewFailed, PreviewReady, PreviewRequest,
    load_preview,
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
            request_rx
                .into_iter()
                .flat_map(|request| {
                    let result = load_preview(&request.path, request.max_bytes);
                    preview_events(&request, result).into_iter()
                })
                .for_each(|event| {
                    let _ = result_tx.send(event);
                });
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

fn build_ready(request: &PreviewRequest, content: PreviewContent) -> PreviewReady {
    PreviewReady {
        id: request.id,
        path: request.path.clone(),
        lines: content.lines,
        truncated: content.truncated,
        reason: content.reason,
        kind_flags: content.kind_flags,
    }
}

fn preview_events(
    request: &PreviewRequest,
    result: Result<PreviewContent, PreviewError>,
) -> Vec<PreviewEvent> {
    match result {
        Ok(content) => vec![
            PreviewEvent::Loading { id: request.id },
            PreviewEvent::Ready(build_ready(request, content)),
        ],
        Err(reason) => vec![
            PreviewEvent::Loading { id: request.id },
            PreviewEvent::Failed(PreviewFailed {
                id: request.id,
                reason,
            }),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn preview_events_returns_loading_then_ready() {
        let request = PreviewRequest {
            id: 1,
            path: PathBuf::from("note.txt"),
            max_bytes: 10,
        };
        let content = PreviewContent {
            lines: vec!["line".to_string()],
            truncated: false,
            reason: None,
            kind_flags: vec![],
        };

        let events = preview_events(&request, Ok(content));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], PreviewEvent::Loading { id: 1 });
        match &events[1] {
            PreviewEvent::Ready(ready) => {
                assert_eq!(ready.id, 1);
                assert_eq!(ready.path, PathBuf::from("note.txt"));
                assert_eq!(ready.lines, vec!["line".to_string()]);
                assert!(!ready.truncated);
                assert_eq!(ready.reason, None);
            }
            _ => panic!("expected ready event"),
        }
    }

    #[test]
    fn preview_events_returns_loading_then_failed() {
        let request = PreviewRequest {
            id: 2,
            path: PathBuf::from("note.txt"),
            max_bytes: 10,
        };

        let events = preview_events(&request, Err(PreviewError::BinaryFile));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0], PreviewEvent::Loading { id: 2 });
        assert_eq!(
            events[1],
            PreviewEvent::Failed(PreviewFailed {
                id: 2,
                reason: PreviewError::BinaryFile,
            })
        );
    }
}
