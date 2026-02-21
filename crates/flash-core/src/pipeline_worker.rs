use crossbeam_channel::{Receiver, Sender};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::pipeline::{LayerKind, PipelineConfig, PipelineResponse};

/// Commands sent to the pipeline worker thread.
enum PipelineCommand {
    Run(PipelineConfig),
    Shutdown,
}

/// Handle for controlling a background pipeline worker.
pub struct PipelineHandle {
    cmd_sender:    Sender<PipelineCommand>,
    resp_receiver: Receiver<PipelineResponse>,
    cancel_flag:   Arc<AtomicBool>,
}

impl PipelineHandle {
    /// Start a new pipeline run with the given config. Cancels any ongoing run first.
    pub fn run(&self, config: PipelineConfig) {
        self.cancel_flag.store(false, Ordering::SeqCst);
        let _ = self.cmd_sender.send(PipelineCommand::Run(config));
    }

    /// Drain all available responses without blocking.
    pub fn try_recv_all(&self) -> Vec<PipelineResponse> {
        let mut results = Vec::new();
        while let Ok(resp) = self.resp_receiver.try_recv() {
            results.push(resp);
        }
        results
    }

    /// Shut down the worker thread.
    pub fn shutdown(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let _ = self.cmd_sender.send(PipelineCommand::Shutdown);
    }
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn a background pipeline worker that operates on the given file data.
pub fn spawn_pipeline_worker(
    file_data:    Arc<dyn AsRef<[u8]> + Send + Sync>,
    line_offsets: Arc<Vec<u64>>,
) -> PipelineHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<PipelineCommand>();
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<PipelineResponse>();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel = cancel_flag.clone();

    thread::spawn(move || {
        let data: &[u8] = (*file_data).as_ref();
        worker_loop(&cmd_rx, &resp_tx, &cancel, data, &line_offsets);
    });

    PipelineHandle {
        cmd_sender:    cmd_tx,
        resp_receiver: resp_rx,
        cancel_flag,
    }
}

fn worker_loop(
    cmd_rx:  &Receiver<PipelineCommand>,
    resp_tx: &Sender<PipelineResponse>,
    cancel:  &Arc<AtomicBool>,
    data:    &[u8],
    offsets: &[u64],
) {
    loop {
        match cmd_rx.recv() {
            Ok(PipelineCommand::Run(config)) => {
                // Drain stale commands, keeping only the latest config
                let mut latest = config;
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        PipelineCommand::Run(c) => latest = c,
                        PipelineCommand::Shutdown => return,
                    }
                }
                cancel.store(false, Ordering::SeqCst);
                run_pipeline(&latest, resp_tx, cancel, data, offsets);
            }
            Ok(PipelineCommand::Shutdown) | Err(_) => break,
        }
    }
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    if bytes.ends_with(b"\r\n") {
        &bytes[..bytes.len() - 2]
    } else if bytes.ends_with(b"\n") {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    }
}

fn run_pipeline(
    config:  &PipelineConfig,
    resp_tx: &Sender<PipelineResponse>,
    cancel:  &Arc<AtomicBool>,
    data:    &[u8],
    offsets: &[u64],
) {
    let line_count = if offsets.len() <= 1 { 0 } else { offsets.len() - 1 };

    // Compile only enabled Filter layers
    let mut filters: Vec<(Regex, bool)> = Vec::new();
    for layer in config.iter() {
        if !layer.enabled { continue; }
        if let LayerKind::Filter { pattern, exclude } = &layer.kind {
            match Regex::new(pattern) {
                Ok(re) => filters.push((re, *exclude)),
                Err(e) => {
                    let _ = resp_tx.send(PipelineResponse::Error(e.to_string()));
                    return;
                }
            }
        }
    }

    let mut passing = Vec::with_capacity(line_count);

    for i in 0..line_count {
        if cancel.load(Ordering::SeqCst) {
            let _ = resp_tx.send(PipelineResponse::Cancelled);
            return;
        }

        let start = offsets[i] as usize;
        let end   = offsets[i + 1] as usize;
        let lb    = trim_newline(&data[start..end]);

        let line_str = match std::str::from_utf8(lb) {
            Ok(s)  => s,
            Err(_) => continue,
        };

        // A line passes if ALL filter layers accept it
        let mut accepted = true;
        for (re, exclude) in &filters {
            let matched = re.is_match(line_str);
            if *exclude && matched  { accepted = false; break; }
            if !exclude && !matched { accepted = false; break; }
        }
        if accepted {
            passing.push(i);
        }
    }

    let _ = resp_tx.send(PipelineResponse::Complete(passing));
}
