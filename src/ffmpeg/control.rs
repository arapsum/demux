use std::fmt;

use tokio::sync::mpsc;

use super::RipPhase;

const CONTROL_CHANNEL_CAPACITY: usize = 4;

/// Reports whether the current platform can suspend an `FFmpeg` process safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCapability {
    /// The process runner can suspend and resume its dedicated process group.
    Supported,
    /// The platform does not provide a process-level implementation yet.
    Unsupported,
}

impl PauseCapability {
    /// Returns the capability available on the current target.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(unix)]
        {
            Self::Supported
        }

        #[cfg(not(unix))]
        {
            Self::Unsupported
        }
    }

    /// Returns whether the current target can expose pause and resume.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }

    /// Explains why pause and resume are unavailable when unsupported.
    #[must_use]
    pub const fn explanation(self) -> Option<&'static str> {
        match self {
            Self::Supported => None,
            Self::Unsupported => {
                Some("Pause and resume are unavailable on this platform; Cancel remains available.")
            }
        }
    }
}

/// Identifies a user-requested process-control operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseControlOperation {
    /// Suspend the active `FFmpeg` process.
    Pause,
    /// Resume a previously suspended `FFmpeg` process.
    Resume,
}

impl fmt::Display for PauseControlOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
        })
    }
}

/// Reports the result of an asynchronous process-control request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseControlEvent {
    /// The process was suspended successfully.
    Paused { phase: RipPhase },
    /// The process resumed successfully.
    Resumed { phase: RipPhase },
    /// The operating system rejected the requested transition.
    Failed {
        operation: PauseControlOperation,
        phase: RipPhase,
        message: String,
    },
}

/// Failure while submitting a pause or resume request to the process runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PauseControlRequestError {
    /// The current target does not support process suspension.
    #[error("pause and resume are unsupported on this platform")]
    Unsupported,
    /// The process runner has already completed or shut down.
    #[error("the active process is no longer accepting pause requests")]
    Closed,
    /// The bounded control channel is already full.
    #[error("the active process is still handling another pause request")]
    Busy,
}

/// GUI-side handle for requesting a pause or resume transition.
#[derive(Debug, Clone)]
pub struct PauseControlHandle {
    capability: PauseCapability,
    sender: mpsc::Sender<PauseControlOperation>,
}

/// Process-side receiver for pause and resume requests.
#[derive(Debug)]
pub struct PauseControlSignal {
    capability: PauseCapability,
    receiver: mpsc::Receiver<PauseControlOperation>,
}

/// Creates a bounded process-control channel for one extraction.
///
/// # Parameters
///
/// - `capability`: Platform capability to expose through the returned handle.
///
/// # Returns
///
/// A GUI-side handle and process-side signal pair.
#[must_use]
pub fn pause_control_pair(capability: PauseCapability) -> (PauseControlHandle, PauseControlSignal) {
    let (sender, receiver) = mpsc::channel(CONTROL_CHANNEL_CAPACITY);
    (
        PauseControlHandle { capability, sender },
        PauseControlSignal {
            capability,
            receiver,
        },
    )
}

impl PauseControlHandle {
    /// Returns the capability captured when this extraction started.
    #[must_use]
    pub const fn capability(&self) -> PauseCapability {
        self.capability
    }

    /// Queues a pause or resume request without blocking the GUI.
    ///
    /// # Parameters
    ///
    /// - `operation`: Transition requested for the active process.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the request was queued.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - The current target does not support process suspension.
    /// - The process runner has closed.
    /// - Another request already occupies the bounded channel.
    pub fn request(
        &self,
        operation: PauseControlOperation,
    ) -> Result<(), PauseControlRequestError> {
        if !self.capability.is_supported() {
            return Err(PauseControlRequestError::Unsupported);
        }

        self.sender
            .try_send(operation)
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => PauseControlRequestError::Busy,
                mpsc::error::TrySendError::Closed(_) => PauseControlRequestError::Closed,
            })
    }
}

impl PauseControlSignal {
    /// Returns the capability captured when this extraction started.
    #[must_use]
    pub const fn capability(&self) -> PauseCapability {
        self.capability
    }

    /// Waits for the next user-requested process-control operation.
    pub(crate) async fn recv(&mut self) -> Option<PauseControlOperation> {
        self.receiver.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_requests_are_bounded_and_idempotency_is_left_to_the_runner() {
        let (handle, mut signal) = pause_control_pair(PauseCapability::Supported);

        assert_eq!(handle.capability(), PauseCapability::Supported);
        assert!(handle.request(PauseControlOperation::Pause).is_ok());
        assert!(handle.request(PauseControlOperation::Resume).is_ok());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            assert_eq!(signal.recv().await, Some(PauseControlOperation::Pause));
            assert_eq!(signal.recv().await, Some(PauseControlOperation::Resume));
        });
    }

    #[test]
    fn unsupported_capabilities_reject_requests_before_process_start() {
        let (handle, _signal) = pause_control_pair(PauseCapability::Unsupported);

        assert_eq!(
            handle.request(PauseControlOperation::Pause),
            Err(PauseControlRequestError::Unsupported)
        );
        assert_eq!(
            PauseCapability::Unsupported.explanation(),
            Some("Pause and resume are unavailable on this platform; Cancel remains available.")
        );
    }
}
