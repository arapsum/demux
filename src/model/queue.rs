use super::{
    encoding::RipOptions,
    job::{JobId, RipJob},
};

/// Allocates job identifiers and retains the latest submitted job.
#[derive(Debug)]
pub(crate) struct JobQueue {
    current: Option<RipJob>,
    next_id: u64,
}

impl JobQueue {
    pub(crate) const fn new() -> Self {
        Self {
            current: None,
            next_id: 1,
        }
    }

    pub(crate) fn create(&mut self, input: String, output: String, options: RipOptions) -> RipJob {
        let id = JobId::new(self.next_id);
        self.next_id += 1;
        RipJob::with_options(id, input, output, options)
    }

    pub(crate) fn finish(&mut self, job: RipJob) {
        self.current = Some(job);
    }

    pub(crate) fn current(&self) -> Option<&RipJob> {
        self.current.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_unique_ids_and_retains_the_latest_job() {
        let mut queue = JobQueue::new();
        let first = queue.create(
            "first.mp4".into(),
            "first.mp3".into(),
            RipOptions::default(),
        );
        let second = queue.create(
            "second.mp4".into(),
            "second.mp3".into(),
            RipOptions::default(),
        );

        assert_eq!(first.id, JobId::new(1));
        assert_eq!(second.id, JobId::new(2));

        queue.finish(second);
        assert_eq!(queue.current().map(|job| &job.id), Some(&JobId::new(2)));
    }
}
