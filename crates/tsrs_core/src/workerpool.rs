//! The compiler's rayon worker pool, configurable from code.
//!
//! `EyeAuras` fork: moved here from `tsrs_compiler` so that the directory walk in `tsrs_vfs` runs on it too.
//!
//! Parsing and binding recurse deeply on large files; Go's goroutine stacks grow on demand, so the worker threads
//! get large stacks. The pool has at most `PARSE_THREAD_CAP` threads by default: with the heap on 4 KiB pages
//! (mimalloc `no_thp`, notes/mem-no-thp.md) the parallel parse is bound by page-fault contention, not threads (vscode
//! on 64 vCPUs: 0.124 s at 32 threads, 0.131 s at 64), and every allocating thread keeps its own partly filled
//! allocator pages and arena chunk tail (64 -> 32 threads: -0.09 GiB peak). `RAYON_NUM_THREADS` (through
//! `hostenv`, ignored with the `embedded` feature) sets the count exactly. An embedding host sets the shape with
//! `configure_worker_pool` before the first compiler call.

use std::num::NonZeroUsize;
use std::sync::{Mutex, OnceLock};

const PARSE_THREAD_CAP: usize = 32;

/// Stack size of a worker thread unless configured.
pub const DEFAULT_WORKER_STACK_SIZE: usize = 256 << 20;

/// Name prefix of the worker threads (`tsrs-worker-0`, ...).
pub const WORKER_THREAD_NAME: &str = "tsrs-worker";

/// Shape of the worker pool; `None` keeps the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkerPoolOptions {
    /// Number of worker threads (default: `RAYON_NUM_THREADS`, else min(available parallelism, 32)).
    pub threads: Option<NonZeroUsize>,
    /// Stack reservation of each worker thread in bytes (default `DEFAULT_WORKER_STACK_SIZE`).
    pub stack_size: Option<usize>,
}

/// `configure_worker_pool` was called after the pool had started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerPoolStarted;

// The configured shape and whether the pool was built. The pool is built under this lock, so a configuration either
// takes effect or reports `WorkerPoolStarted`.
static CONFIG: Mutex<(WorkerPoolOptions, bool)> =
    Mutex::new((WorkerPoolOptions { threads: None, stack_size: None }, false));
static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Sets the pool's thread count and stack size.
///
/// Must run before the first compiler call that uses the pool; afterwards it returns `WorkerPoolStarted` and
/// changes nothing. The pool lives until the process exits.
pub fn configure_worker_pool(options: WorkerPoolOptions) -> Result<(), WorkerPoolStarted> {
    let mut config = CONFIG.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if config.1 {
        return Err(WorkerPoolStarted);
    }
    config.0 = options;
    Ok(())
}

/// Whether the pool has been built.
pub fn worker_pool_started() -> bool {
    POOL.get().is_some()
}

/// Runs `op` in a rayon scope on the compiler's pool.
///
/// Inside a rayon pool the scope stays there; outside, it uses the compiler's pool, so work never lands on rayon's
/// implicit global pool. A host that calls the compiler from a thread of its own rayon pool runs this work (the
/// include-glob directory walk) on that pool.
pub fn scope<'scope, OP, R>(op: OP) -> R
where
    OP: FnOnce(&rayon::Scope<'scope>) -> R + Send,
    R: Send,
{
    if rayon::current_thread_index().is_some() { rayon::scope(op) } else { worker_pool().scope(op) }
}

/// The compiler's worker pool, built on first use.
///
/// `NO_THREADS` targets: a one-thread pool on the calling thread (`use_current_thread`), on which
/// `install`, `par_iter`, `broadcast` and nested scopes run inline. It must be the first rayon use on that
/// thread: once rayon's global pool has started there, building it fails with "The current thread is already
/// part of another thread pool", so an embedder calls `worker_pool()` before anything else.
pub fn worker_pool() -> &'static rayon::ThreadPool {
    POOL.get_or_init(|| {
        // A failed build leaves the lock usable and the pool unbuilt (the next call retries).
        let mut config = CONFIG.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if crate::NO_THREADS {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(1).use_current_thread().build();
            config.1 = pool.is_ok();
            drop(config);
            return pool.expect("building the compiler's worker pool");
        }
        let threads = config.0.threads.map_or_else(
            || {
                crate::hostenv::var("RAYON_NUM_THREADS")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|&n| n > 0)
                    .unwrap_or_else(|| {
                        std::thread::available_parallelism()
                            .map_or(1, NonZeroUsize::get)
                            .min(PARSE_THREAD_CAP)
                    })
            },
            NonZeroUsize::get,
        );
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .stack_size(config.0.stack_size.unwrap_or(DEFAULT_WORKER_STACK_SIZE))
            .thread_name(|i| format!("{WORKER_THREAD_NAME}-{i}"))
            .build();
        config.1 = pool.is_ok();
        drop(config);
        pool.expect("building the compiler's worker pool")
    })
}
