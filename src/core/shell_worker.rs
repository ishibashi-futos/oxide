use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Instant, SystemTime};

use crate::core::{ShellCommandRequest, ShellExecutionGuard, ShellExecutionResult};

const MAX_CAPTURE_BYTES: usize = 2_097_152;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    Stdout(String),
    Stderr(String),
    Finished(ShellExecutionResult),
    Failed(String),
}

#[derive(Debug)]
pub struct ShellWorker {
    request_tx: Sender<ShellCommandRequest>,
    event_rx: Receiver<ShellEvent>,
}

impl ShellWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<ShellCommandRequest>();
        let (event_tx, event_rx) = mpsc::channel::<ShellEvent>();

        thread::spawn(move || {
            for request in request_rx {
                let guard = ShellExecutionGuard::new();
                let start = Instant::now();
                let timestamp_ms = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0);
                let mut command = guard.build_command(&request);
                command.stdout(Stdio::piped()).stderr(Stdio::piped());

                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(error) => {
                        let _ =
                            event_tx.send(ShellEvent::Failed(format!("shell: {error}")));
                        continue;
                    }
                };

                let stdout_handle = child
                    .stdout
                    .take()
                    .map(|stdout| spawn_reader(stdout, event_tx.clone(), StreamKind::Stdout));
                let stderr_handle = child
                    .stderr
                    .take()
                    .map(|stderr| spawn_reader(stderr, event_tx.clone(), StreamKind::Stderr));

                let status = child.wait().ok();
                let stdout = stdout_handle
                    .and_then(|handle| handle.join().ok())
                    .unwrap_or_default();
                let stderr = stderr_handle
                    .and_then(|handle| handle.join().ok())
                    .unwrap_or_default();

                let result = ShellExecutionResult {
                    status_code: status.and_then(|status| status.code()),
                    stdout: String::from_utf8_lossy(&stdout).to_string(),
                    stderr: String::from_utf8_lossy(&stderr).to_string(),
                    duration_ms: start.elapsed().as_millis(),
                    timestamp_ms,
                };
                let _ = event_tx.send(ShellEvent::Finished(result));
            }
        });

        Self {
            request_tx,
            event_rx,
        }
    }

    pub fn request(&self, request: ShellCommandRequest) {
        let _ = self.request_tx.send(request);
    }

    pub fn poll(&self) -> Option<ShellEvent> {
        self.event_rx.try_recv().ok()
    }
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    event_tx: Sender<ShellEvent>,
    kind: StreamKind,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut collected = Vec::new();
        let mut total = 0usize;
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            let read = match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => break,
            };
            if read == 0 {
                break;
            }
            let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
            let event = match kind {
                StreamKind::Stdout => ShellEvent::Stdout(trimmed),
                StreamKind::Stderr => ShellEvent::Stderr(trimmed),
            };
            let _ = event_tx.send(event);
            if total < MAX_CAPTURE_BYTES {
                let remaining = MAX_CAPTURE_BYTES - total;
                let bytes = line.as_bytes();
                let take = remaining.min(bytes.len());
                collected.extend_from_slice(&bytes[..take]);
                total += take;
            }
        }
        collected
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn shell_worker_streams_stdout_lines() {
        let temp_dir = tempfile::tempdir().unwrap();
        let request = ShellCommandRequest::new(
            temp_dir.path().to_path_buf(),
            "printf 'one\\ntwo\\n'",
        )
        .unwrap();
        let worker = ShellWorker::new();

        worker.request(request);

        let mut stdout_lines = Vec::new();
        let mut finished = None;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if let Some(event) = worker.poll() {
                match event {
                    ShellEvent::Stdout(line) => stdout_lines.push(line),
                    ShellEvent::Finished(result) => {
                        finished = Some(result);
                        break;
                    }
                    ShellEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }

        let result = finished.expect("shell result");
        assert_eq!(stdout_lines, vec!["one".to_string(), "two".to_string()]);
        assert!(result.stdout.contains("one"));
        assert!(result.stdout.contains("two"));
    }
}
