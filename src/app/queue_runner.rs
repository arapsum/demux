use std::collections::VecDeque;

use crate::model::job::JobId;

/// Terminal counts produced by one queue execution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueRunSummary {
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
}

/// Coordinates one deterministic, single-process queue execution.
#[derive(Debug)]
pub struct QueueRunner {
    pending: VecDeque<JobId>,
    active: Option<JobId>,
    total: usize,
    summary: QueueRunSummary,
    cancelling: bool,
}

impl QueueRunner {
    #[must_use]
    pub fn new(eligible: impl IntoIterator<Item = JobId>, skipped: usize) -> Option<Self> {
        let pending: VecDeque<_> = eligible.into_iter().collect();
        if pending.is_empty() {
            return None;
        }

        Some(Self {
            total: pending.len(),
            pending,
            active: None,
            summary: QueueRunSummary {
                skipped,
                ..QueueRunSummary::default()
            },
            cancelling: false,
        })
    }

    #[must_use]
    pub fn start_next(&mut self) -> Option<JobId> {
        if self.active.is_some() || self.cancelling {
            return None;
        }
        self.active = self.pending.pop_front();
        self.active.clone()
    }

    #[must_use]
    pub const fn active(&self) -> Option<&JobId> {
        self.active.as_ref()
    }

    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    #[must_use]
    pub fn position(&self) -> Option<usize> {
        self.active.as_ref()?;
        Some(self.summary.completed + self.summary.failed + 1)
    }

    pub fn finish_active(&mut self, job_id: &JobId, succeeded: bool) -> bool {
        if self.active.as_ref() != Some(job_id) {
            return false;
        }

        self.active = None;
        if succeeded {
            self.summary.completed += 1;
        } else {
            self.summary.failed += 1;
        }
        true
    }

    /// Stops queue advancement and returns the jobs that will never be started.
    pub fn request_cancel(&mut self, job_id: &JobId) -> Option<Vec<JobId>> {
        if self.cancelling || self.active.as_ref() != Some(job_id) {
            return None;
        }

        self.cancelling = true;
        let pending: Vec<_> = self.pending.drain(..).collect();
        self.summary.cancelled += pending.len();
        Some(pending)
    }

    pub fn finish_cancelled(&mut self, job_id: &JobId) -> bool {
        if !self.cancelling || self.active.as_ref() != Some(job_id) {
            return false;
        }

        self.active = None;
        self.summary.cancelled += 1;
        true
    }

    #[must_use]
    pub const fn is_cancelling(&self) -> bool {
        self.cancelling
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.active.is_none() && self.pending.is_empty()
    }

    #[must_use]
    pub const fn summary(&self) -> QueueRunSummary {
        self.summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_ids_in_order_and_rejects_stale_results() {
        let first = JobId::new(1);
        let second = JobId::new(2);
        let mut runner = QueueRunner::new([first.clone(), second.clone()], 1).unwrap();

        assert_eq!(runner.start_next(), Some(first.clone()));
        assert_eq!(runner.position(), Some(1));
        assert_eq!(runner.start_next(), None);
        assert!(!runner.finish_active(&second, true));
        assert!(runner.finish_active(&first, true));

        assert_eq!(runner.start_next(), Some(second.clone()));
        assert_eq!(runner.position(), Some(2));
        assert!(runner.finish_active(&second, false));
        assert!(runner.is_finished());
        assert_eq!(
            runner.summary(),
            QueueRunSummary {
                completed: 1,
                failed: 1,
                skipped: 1,
                cancelled: 0,
            }
        );
    }

    #[test]
    fn empty_execution_is_not_started() {
        assert!(QueueRunner::new([], 2).is_none());
    }

    #[test]
    fn cancellation_is_idempotent_and_accounts_for_active_and_pending_jobs() {
        let first = JobId::new(1);
        let second = JobId::new(2);
        let third = JobId::new(3);
        let mut runner =
            QueueRunner::new([first.clone(), second.clone(), third.clone()], 0).unwrap();
        assert_eq!(runner.start_next(), Some(first.clone()));

        assert_eq!(runner.request_cancel(&first), Some(vec![second, third]));
        assert_eq!(runner.request_cancel(&first), None);
        assert!(runner.is_cancelling());
        assert_eq!(runner.start_next(), None);
        assert!(runner.finish_cancelled(&first));
        assert!(!runner.finish_cancelled(&first));
        assert!(runner.is_finished());
        assert_eq!(
            runner.summary(),
            QueueRunSummary {
                cancelled: 3,
                ..QueueRunSummary::default()
            }
        );
    }
}
