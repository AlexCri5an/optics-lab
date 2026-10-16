//! Background worker task abstraction with progress reporting, cancellation, and channel interfaces.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use serde::{Deserialize, Serialize};

/// Progress update or final outcome message emitted by a background worker job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobMessage<T> {
    /// Incremental progress update.
    Progress {
        /// Completion percentage in range [0.0, 100.0].
        percent: f32,
        /// Informational status text.
        status: String,
    },
    /// The worker finished computation successfully with output.
    Completed(T),
    /// The worker aborted due to an error.
    Failed(String),
    /// The worker was cleanly cancelled via cancellation token.
    Cancelled,
}

/// A handle for monitoring and controlling an active background task.
#[derive(Debug)]
pub struct JobHandle<T> {
    job_id: u64,
    cancel_flag: Arc<AtomicBool>,
    rx: Receiver<JobMessage<T>>,
}

impl<T: Send + 'static> JobHandle<T> {
    /// Spawns a background worker thread executing a computational task.
    pub fn spawn<F>(job_id: u64, task: F) -> Self
    where
        F: FnOnce(Arc<AtomicBool>, Sender<JobMessage<T>>) + Send + 'static,
    {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel_flag);
        let (tx, rx) = channel();

        thread::spawn(move || {
            task(cancel_clone, tx);
        });

        Self {
            job_id,
            cancel_flag,
            rx,
        }
    }

    /// Returns the unique job ID.
    pub fn id(&self) -> u64 {
        self.job_id
    }

    /// Signals the worker thread to cancel processing.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Checks if a cancellation request has been signaled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }

    /// Receives messages without blocking (polling).
    pub fn try_recv(&self) -> Result<JobMessage<T>, std::sync::mpsc::TryRecvError> {
        self.rx.try_recv()
    }

    /// Waits synchronously for the next message from the worker.
    pub fn recv(&self) -> Result<JobMessage<T>, std::sync::mpsc::RecvError> {
        self.rx.recv()
    }
}
