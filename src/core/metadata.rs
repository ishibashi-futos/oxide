use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMetadata {
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MetadataStatus {
    Loading,
    Error,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FetchPriority {
    High,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRequest {
    pub request_id: RequestId,
    pub path: PathBuf,
    pub priority: FetchPriority,
}

#[derive(Debug)]
pub struct FetchQueue {
    max_in_flight: usize,
    in_flight: Vec<RequestId>,
    high: VecDeque<MetadataRequest>,
    low: VecDeque<MetadataRequest>,
}

impl FetchQueue {
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight: max_in_flight.max(1),
            in_flight: Vec::new(),
            high: VecDeque::new(),
            low: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, request: MetadataRequest) {
        match request.priority {
            FetchPriority::High => self.high.push_back(request),
            FetchPriority::Low => self.low.push_back(request),
        }
    }

    pub fn start_next(&mut self) -> Option<MetadataRequest> {
        if self.in_flight.len() >= self.max_in_flight {
            return None;
        }
        let request = if let Some(request) = self.high.pop_front() {
            request
        } else {
            self.low.pop_front()?
        };
        self.in_flight.push(request.request_id);
        Some(request)
    }

    pub fn cancel(&mut self, request_id: RequestId) -> bool {
        if Self::remove_from_queue(&mut self.high, request_id)
            || Self::remove_from_queue(&mut self.low, request_id)
        {
            return true;
        }
        if let Some(index) = self
            .in_flight
            .iter()
            .position(|candidate| *candidate == request_id)
        {
            self.in_flight.swap_remove(index);
            return true;
        }
        false
    }

    pub fn complete(&mut self, request_id: RequestId) {
        if let Some(index) = self
            .in_flight
            .iter()
            .position(|candidate| *candidate == request_id)
        {
            self.in_flight.swap_remove(index);
        }
    }

    fn remove_from_queue(queue: &mut VecDeque<MetadataRequest>, request_id: RequestId) -> bool {
        if let Some(index) = queue
            .iter()
            .position(|request| request.request_id == request_id)
        {
            queue.remove(index);
            return true;
        }
        false
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequestId(u64);

impl RequestId {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

pub struct RequestTracker {
    current: RequestId,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self {
            current: RequestId::new(),
        }
    }

    pub fn next(&mut self) -> RequestId {
        self.current = self.current.next();
        self.current
    }

    pub fn is_latest(&self, request_id: RequestId) -> bool {
        request_id == self.current
    }
}

pub fn entry_metadata(path: &Path) -> AppResult<EntryMetadata> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(EntryMetadata {
        size: metadata.len(),
        modified,
    })
}

#[allow(dead_code)]
pub struct MetadataFetchResult {
    pub request_id: RequestId,
    pub path: PathBuf,
    pub metadata: AppResult<EntryMetadata>,
}

#[cfg(test)]
pub trait MetadataFetcher {
    fn fetch(&self, request_id: RequestId, path: PathBuf) -> MetadataFetchResult;
}

pub struct MetadataSnapshot {
    entries: HashMap<PathBuf, EntryMetadata>,
}

impl MetadataSnapshot {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn apply(&mut self, result: MetadataFetchResult) {
        if let Ok(metadata) = result.metadata {
            self.entries.insert(result.path, metadata);
        }
    }

    pub fn get(&self, path: &Path) -> Option<&EntryMetadata> {
        self.entries.get(path)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
pub struct FakeMetadataFetcher {
    responses: HashMap<PathBuf, EntryMetadata>,
}

#[cfg(test)]
impl FakeMetadataFetcher {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, path: PathBuf, metadata: EntryMetadata) -> Self {
        self.responses.insert(path, metadata);
        self
    }
}

#[cfg(test)]
impl MetadataFetcher for FakeMetadataFetcher {
    fn fetch(&self, request_id: RequestId, path: PathBuf) -> MetadataFetchResult {
        let metadata = self
            .responses
            .get(&path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing metadata"))
            .map_err(Into::into);

        MetadataFetchResult {
            request_id,
            path,
            metadata,
        }
    }
}

pub struct MetadataWindow<T> {
    items: VecDeque<T>,
}

impl<T: Clone> MetadataWindow<T> {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }

    pub fn refresh(&mut self, source: &[T], selected_index: usize) {
        self.items.clear();
        if source.is_empty() || selected_index >= source.len() {
            return;
        }

        let start = selected_index.saturating_sub(5);
        let end = (selected_index + 5).min(source.len() - 1);
        self.items.extend(source[start..=end].iter().cloned());
    }

    pub fn items(&self) -> &VecDeque<T> {
        &self.items
    }
}

#[allow(dead_code)]
pub fn prefetch_paths(source: &[PathBuf], selected_index: usize) -> Vec<PathBuf> {
    if source.is_empty() || selected_index >= source.len() {
        return Vec::new();
    }
    let start = selected_index.saturating_sub(5);
    let end = (selected_index + 5).min(source.len() - 1);
    source[start..=end].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn entry_metadata_returns_size_and_modified() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let metadata = entry_metadata(&file_path).unwrap();

        assert_eq!(metadata.size, 5);
        assert!(metadata.modified >= UNIX_EPOCH);
    }

    #[test]
    fn request_id_increments_monotonically() {
        let first = RequestId::new();
        let second = first.next();
        let third = second.next();

        assert!(first < second);
        assert!(second < third);
    }

    #[test]
    fn fake_fetcher_ignores_results_from_old_request_id() {
        let mut tracker = RequestTracker::new();
        let first = tracker.next();
        let second = tracker.next();

        let path = PathBuf::from("alpha.txt");
        let metadata = EntryMetadata {
            size: 10,
            modified: UNIX_EPOCH,
        };
        let fetcher = FakeMetadataFetcher::new().with_metadata(path.clone(), metadata);

        let old_result = fetcher.fetch(first, path.clone());
        let new_result = fetcher.fetch(second, path);

        assert!(!tracker.is_latest(old_result.request_id));
        assert!(tracker.is_latest(new_result.request_id));
    }

    #[test]
    fn metadata_snapshot_collects_results_for_request() {
        let request_id = RequestTracker::new().next();
        let first_path = PathBuf::from("alpha.txt");
        let second_path = PathBuf::from("beta.txt");
        let first_metadata = EntryMetadata {
            size: 1,
            modified: UNIX_EPOCH,
        };
        let second_metadata = EntryMetadata {
            size: 2,
            modified: UNIX_EPOCH,
        };
        let fetcher = FakeMetadataFetcher::new()
            .with_metadata(first_path.clone(), first_metadata.clone())
            .with_metadata(second_path.clone(), second_metadata.clone());

        let mut snapshot = MetadataSnapshot::new();
        snapshot.apply(fetcher.fetch(request_id, first_path.clone()));
        snapshot.apply(fetcher.fetch(request_id, second_path.clone()));

        assert_eq!(snapshot.get(&first_path), Some(&first_metadata));
        assert_eq!(snapshot.get(&second_path), Some(&second_metadata));
    }

    #[test]
    fn prefetch_paths_returns_selected_plus_minus_five() {
        let source: Vec<PathBuf> = (0..20)
            .map(|index| PathBuf::from(format!("file-{index}")))
            .collect();

        let paths = prefetch_paths(&source, 10);
        let names: Vec<String> = paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            names,
            (5..=15)
                .map(|index| format!("file-{index}"))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn fetch_queue_prioritizes_high_over_low() {
        let mut tracker = RequestTracker::new();
        let low_id = tracker.next();
        let high_id = tracker.next();
        let low = MetadataRequest {
            request_id: low_id,
            path: PathBuf::from("low.txt"),
            priority: FetchPriority::Low,
        };
        let high = MetadataRequest {
            request_id: high_id,
            path: PathBuf::from("high.txt"),
            priority: FetchPriority::High,
        };
        let mut queue = FetchQueue::new(1);

        queue.enqueue(low);
        queue.enqueue(high);

        let started = queue.start_next().unwrap();
        assert_eq!(started.request_id, high_id);
    }

    #[test]
    fn fetch_queue_cancels_in_flight_and_releases_permit() {
        let mut tracker = RequestTracker::new();
        let low_id = tracker.next();
        let high_id = tracker.next();
        let low = MetadataRequest {
            request_id: low_id,
            path: PathBuf::from("low.txt"),
            priority: FetchPriority::Low,
        };
        let high = MetadataRequest {
            request_id: high_id,
            path: PathBuf::from("high.txt"),
            priority: FetchPriority::High,
        };
        let mut queue = FetchQueue::new(1);

        queue.enqueue(low);
        let started = queue.start_next().unwrap();
        assert_eq!(started.request_id, low_id);

        assert!(queue.cancel(low_id));

        queue.enqueue(high);
        let started = queue.start_next().unwrap();
        assert_eq!(started.request_id, high_id);
    }

    #[test]
    fn fetch_queue_completes_and_releases_permit() {
        let mut tracker = RequestTracker::new();
        let first_id = tracker.next();
        let second_id = tracker.next();
        let first = MetadataRequest {
            request_id: first_id,
            path: PathBuf::from("first.txt"),
            priority: FetchPriority::Low,
        };
        let second = MetadataRequest {
            request_id: second_id,
            path: PathBuf::from("second.txt"),
            priority: FetchPriority::High,
        };
        let mut queue = FetchQueue::new(1);

        queue.enqueue(first);
        let started = queue.start_next().unwrap();
        assert_eq!(started.request_id, first_id);
        queue.complete(first_id);

        queue.enqueue(second);
        let started = queue.start_next().unwrap();
        assert_eq!(started.request_id, second_id);
    }

    #[test]
    fn metadata_window_keeps_selected_plus_minus_five() {
        let source: Vec<u8> = (0..20).collect();
        let mut window = MetadataWindow::new();

        window.refresh(&source, 10);

        let items: Vec<u8> = window.items().iter().copied().collect();
        assert_eq!(items, (5..=15).collect::<Vec<u8>>());
        assert!(window.items().len() <= 11);
    }

    #[test]
    fn metadata_window_drops_items_outside_range() {
        let source: Vec<u8> = (0..20).collect();
        let mut window = MetadataWindow::new();

        window.refresh(&source, 3);
        let items: Vec<u8> = window.items().iter().copied().collect();
        assert_eq!(items, (0..=8).collect::<Vec<u8>>());

        window.refresh(&source, 18);
        let items: Vec<u8> = window.items().iter().copied().collect();
        assert_eq!(items, (13..=19).collect::<Vec<u8>>());
        assert!(window.items().len() <= 11);
    }
}
