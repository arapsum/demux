use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Notify;

/// Shared atomic state backing one cancellation handle and signal pair.
#[derive(Debug, Default)]
struct CancellationState {
    requested: AtomicBool,
    notified: Notify,
}

/// A clonable control handle for requesting cancellation of one `FFmpeg` process.
#[derive(Debug, Clone)]
pub struct CancellationHandle {
    state: Arc<CancellationState>,
}

/// The process-side signal paired with a [`CancellationHandle`].
#[derive(Debug, Clone)]
pub struct CancellationSignal {
    state: Arc<CancellationState>,
}

/// Creates the control and process sides of one cancellation request.
#[must_use]
pub fn cancellation_pair() -> (CancellationHandle, CancellationSignal) {
    let state = Arc::new(CancellationState::default());
    (
        CancellationHandle {
            state: state.clone(),
        },
        CancellationSignal { state },
    )
}

impl CancellationHandle {
    /// Requests cancellation, returning `true` only for the first request.
    #[must_use]
    pub fn cancel(&self) -> bool {
        if self.state.requested.swap(true, Ordering::AcqRel) {
            return false;
        }

        self.state.notified.notify_waiters();
        true
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.requested.load(Ordering::Acquire)
    }
}

impl CancellationSignal {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.requested.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        loop {
            let notified = self.state.notified.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_is_idempotent_and_observable_after_the_request() {
        let (handle, signal) = cancellation_pair();

        assert!(!handle.is_cancelled());
        assert!(handle.cancel());
        assert!(!handle.cancel());
        signal.cancelled().await;
        assert!(signal.is_cancelled());
    }
}
