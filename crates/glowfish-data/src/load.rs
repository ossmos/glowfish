use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::DataError;

/// The threads that read and reduce source data.
///
/// Loading is limited by memory (or disk) bandwidth, not CPU: beyond a few threads they only
/// contend, and a whole-file pass gets slower. Pyramids can share one pool.
#[derive(Clone)]
pub struct LoadPool(Arc<rayon::ThreadPool>);

impl LoadPool {
    /// A pool with `threads` threads; 0 picks [`default_threads`](Self::default_threads).
    pub fn new(threads: usize) -> Self {
        let threads = if threads == 0 {
            Self::default_threads()
        } else {
            threads
        };
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("glowfish-load-{i}"))
            .build()
            .expect("creating the load thread pool failed");
        Self(Arc::new(pool))
    }

    /// 4 threads, or fewer on smaller machines.
    pub fn default_threads() -> usize {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
    }

    pub fn threads(&self) -> usize {
        self.0.current_num_threads()
    }

    pub(crate) fn install<R: Send>(&self, f: impl FnOnce() -> R + Send) -> R {
        self.0.install(f)
    }
}

impl Default for LoadPool {
    fn default() -> Self {
        Self::new(0)
    }
}

impl std::fmt::Debug for LoadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("LoadPool").field(&self.threads()).finish()
    }
}

/// Cancels a running query and reports its progress. Clones share the same state, so one clone
/// can be handed to the loading thread and another kept by the UI.
#[derive(Debug, Clone, Default)]
pub struct LoadControl(Arc<ControlState>);

#[derive(Debug, Default)]
struct ControlState {
    cancelled: AtomicBool,
    total: AtomicU64,
    done: AtomicU64,
}

impl LoadControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the query at the next chunk of rows; it returns [`DataError::Cancelled`]. Tiles
    /// completed so far stay cached.
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Relaxed)
    }

    /// Rows of the query, including rows that are already cached.
    pub fn rows_total(&self) -> u64 {
        self.0.total.load(Ordering::Relaxed)
    }

    /// Rows that were read or found in the cache so far.
    pub fn rows_done(&self) -> u64 {
        self.0.done.load(Ordering::Relaxed).min(self.rows_total())
    }

    /// `0.0..=1.0`; 1 before a query started.
    pub fn progress(&self) -> f32 {
        match self.rows_total() {
            0 => 1.0,
            total => self.rows_done() as f32 / total as f32,
        }
    }

    pub(crate) fn start(&self, total: u64) {
        self.0.done.store(0, Ordering::Relaxed);
        self.0.total.store(total, Ordering::Relaxed);
    }

    pub(crate) fn finish(&self) {
        self.0.done.store(self.rows_total(), Ordering::Relaxed);
    }

    pub(crate) fn check(&self) -> Result<(), DataError> {
        if self.is_cancelled() {
            return Err(DataError::Cancelled);
        }
        Ok(())
    }
}

/// A running query: its rows and its control.
pub(crate) struct Load<'a> {
    pub control: &'a LoadControl,
    pub query: Range<u64>,
}

impl Load<'_> {
    /// Counts the queried rows in `rows` as done.
    pub fn advance(&self, rows: &Range<u64>) {
        let start = rows.start.max(self.query.start);
        let end = rows.end.min(self.query.end);
        if end > start {
            self.control
                .0
                .done
                .fetch_add(end - start, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_and_cancel() {
        let control = LoadControl::new();
        assert_eq!(control.progress(), 1.0);
        control.start(100);
        let load = Load {
            control: &control,
            query: 10..110,
        };
        load.advance(&(0..30));
        assert_eq!(control.rows_done(), 20);
        assert_eq!(control.progress(), 0.2);
        load.advance(&(0..1000));
        assert_eq!(control.rows_done(), 100);
        assert!(control.check().is_ok());
        control.clone().cancel();
        assert!(matches!(control.check(), Err(DataError::Cancelled)));
    }

    #[test]
    fn pool_threads() {
        assert_eq!(LoadPool::new(2).threads(), 2);
        assert!(LoadPool::default().threads() <= 4);
    }
}
