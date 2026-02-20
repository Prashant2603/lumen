use crossbeam_channel::{Receiver, Sender};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// A single search match.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub line_number: usize,
    pub line_text: String,
}

/// Commands sent to the search worker thread.
pub enum SearchCommand {
    /// Start a new search with the given regex pattern.
    Search(String),
    /// Shut down the worker.
    Shutdown,
}

/// Messages sent from the search worker back to the UI.
#[derive(Debug, Clone)]
pub enum SearchResponse {
    /// A batch of search results.
    Batch(Vec<SearchResult>),
    /// Search completed (total matches found).
    Complete(usize),
    /// Search was cancelled.
    Cancelled,
    /// Error compiling regex or during search.
    Error(String),
}

/// Handle for controlling a background search.
pub struct SearchHandle {
    cmd_sender: Sender<SearchCommand>,
    pub response_receiver: Receiver<SearchResponse>,
    cancel_flag: Arc<AtomicBool>,
}

impl SearchHandle {
    /// Start a new search. Cancels any ongoing search first.
    pub fn search(&self, pattern: String) {
        self.cancel_flag.store(false, Ordering::SeqCst);
        let _ = self.cmd_sender.send(SearchCommand::Search(pattern));
    }

    /// Cancel the current search.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Shut down the worker thread.
    pub fn shutdown(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let _ = self.cmd_sender.send(SearchCommand::Shutdown);
    }

    /// Drain available responses without blocking.
    pub fn try_recv_all(&self) -> Vec<SearchResponse> {
        let mut results = Vec::new();
        while let Ok(resp) = self.response_receiver.try_recv() {
            results.push(resp);
        }
        results
    }
}

impl Drop for SearchHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn a background search worker that operates on the given file data.
///
/// `file_data` is a trait object so it can be backed by either a memory-mapped
/// file (`Arc<Mmap>`) or a decompressed buffer (`Arc<Vec<u8>>`).  The Arc keeps
/// the data alive for the full lifetime of the worker thread.
pub fn spawn_search_worker(
    file_data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    line_offsets: Arc<Vec<u64>>,
) -> SearchHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<SearchCommand>();
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<SearchResponse>();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel = cancel_flag.clone();

    thread::spawn(move || {
        // Obtain &[u8] from the trait object; lifetime is tied to the Arc.
        let data: &[u8] = (*file_data).as_ref();
        worker_loop(&cmd_rx, &resp_tx, &cancel, data, &line_offsets);
    });

    SearchHandle {
        cmd_sender: cmd_tx,
        response_receiver: resp_rx,
        cancel_flag,
    }
}

fn worker_loop(
    cmd_rx: &Receiver<SearchCommand>,
    resp_tx: &Sender<SearchResponse>,
    cancel: &Arc<AtomicBool>,
    data: &[u8],
    offsets: &[u64],
) {
    loop {
        match cmd_rx.recv() {
            Ok(SearchCommand::Search(pattern)) => {
                run_search(&pattern, resp_tx, cancel, data, offsets);
            }
            Ok(SearchCommand::Shutdown) | Err(_) => break,
        }
        // Drain any queued search commands, keeping only the last one
        let mut latest: Option<String> = None;
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                SearchCommand::Search(p) => latest = Some(p),
                SearchCommand::Shutdown => return,
            }
        }
        if let Some(pattern) = latest {
            cancel.store(false, Ordering::SeqCst);
            run_search(&pattern, resp_tx, cancel, data, offsets);
        }
    }
}

fn run_search(
    pattern: &str,
    resp_tx: &Sender<SearchResponse>,
    cancel: &Arc<AtomicBool>,
    data: &[u8],
    offsets: &[u64],
) {
    let regex = match Regex::new(pattern) {
        Ok(r) => r,
        Err(e) => {
            let _ = resp_tx.send(SearchResponse::Error(e.to_string()));
            return;
        }
    };

    let line_count = if offsets.len() <= 1 { 0 } else { offsets.len() - 1 };
    let mut batch = Vec::with_capacity(1000);
    let mut total_matches = 0;
    let batch_size = 1000;

    for i in 0..line_count {
        if cancel.load(Ordering::SeqCst) {
            let _ = resp_tx.send(SearchResponse::Cancelled);
            return;
        }

        let start = offsets[i] as usize;
        let end = offsets[i + 1] as usize;
        let line_bytes = &data[start..end];

        // Trim trailing newline
        let trimmed = if line_bytes.ends_with(b"\r\n") {
            &line_bytes[..line_bytes.len() - 2]
        } else if line_bytes.ends_with(b"\n") {
            &line_bytes[..line_bytes.len() - 1]
        } else {
            line_bytes
        };

        if let Ok(line_str) = std::str::from_utf8(trimmed) {
            if regex.is_match(line_str) {
                batch.push(SearchResult {
                    line_number: i,
                    line_text: line_str.to_string(),
                });
                total_matches += 1;

                if batch.len() >= batch_size {
                    let _ = resp_tx.send(SearchResponse::Batch(std::mem::take(&mut batch)));
                    batch = Vec::with_capacity(batch_size);
                }
            }
        }
    }

    if !batch.is_empty() {
        let _ = resp_tx.send(SearchResponse::Batch(batch));
    }
    let _ = resp_tx.send(SearchResponse::Complete(total_matches));
}
