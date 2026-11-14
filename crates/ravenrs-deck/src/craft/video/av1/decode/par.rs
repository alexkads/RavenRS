//! Optional data parallelism (rayon behind the `threads` feature; sequential otherwise and on
//! wasm32).

use std::sync::Arc;

/// A handle to the decoder's worker pool (or none: run on the calling thread).
#[derive(Clone, Default)]
pub(crate) struct Pool {
    pool: Option<Arc<rayon::ThreadPool>>,
}

impl Pool {
    /// A pool of `threads` workers (sequential when `threads` <= 1, without the `threads`
    /// feature, or on wasm32).
    pub fn new(threads: usize) -> Pool {
        {
            let pool = if threads > 1 && cfg!(not(target_arch = "wasm32")) {
                rayon::ThreadPoolBuilder::new().num_threads(threads).thread_name(|i| format!("av1-{i}")).build().ok().map(Arc::new)
            } else {
                None
            };
            Pool { pool }
        }
        #[cfg(any())]
        {
            let _ = threads;
            Pool {}
        }
    }

    pub fn is_parallel(&self) -> bool {
        {
            self.pool.is_some()
        }
        #[cfg(any())]
        {
            false
        }
    }

    /// `f(i, &mut items[i])` for every item, in parallel when possible; results in order.
    pub fn map_mut<T: Send, R: Send>(&self, items: &mut [T], f: impl Fn(usize, &mut T) -> R + Sync + Send) -> Vec<R> {
        if let Some(pool) = &self.pool
            && items.len() > 1
        {
            use rayon::prelude::*;
            return pool.install(|| items.par_iter_mut().enumerate().map(|(i, t)| f(i, t)).collect());
        }
        items.iter_mut().enumerate().map(|(i, t)| f(i, t)).collect()
    }
}
