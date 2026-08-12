use std::path::{Path, PathBuf};

use crate::{
    ffmpeg::DependencyState,
    model::job::{JobId, JobStatus, RipJob},
};

use super::output_settings::OutputSettings;
use super::toast::Notifications;

#[derive(Debug)]
pub struct Demux {
    pub(crate) dependency_state: DependencyState,
    pub(crate) jobs: Vec<RipJob>,
    pub(crate) selected_job: Option<JobId>,
    pub(crate) output_settings: OutputSettings,
    pub(crate) error: Option<String>,
    pub(crate) picking_file: bool,
    pub(crate) notifications: Notifications,
    next_job_id: u64,
}

impl Default for Demux {
    fn default() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            jobs: Vec::new(),
            selected_job: None,
            output_settings: OutputSettings::new(),
            error: None,
            picking_file: false,
            notifications: Notifications::new(),
            next_job_id: 1,
        }
    }
}

impl Demux {
    pub(crate) fn add_job(&mut self, input: PathBuf) -> JobId {
        let id = JobId::new(self.next_job_id);
        self.next_job_id += 1;

        self.output_settings.set_default_from_input(&input);
        let output = self.output_settings.output_path(&input);
        let mut job = RipJob::new(
            id.clone(),
            input.to_string_lossy().into_owned(),
            output.to_string_lossy().into_owned(),
        );
        job.start_probing();

        self.jobs.clear();
        self.jobs.push(job);
        self.selected_job = Some(id.clone());
        id
    }

    pub(crate) fn selected_job(&self) -> Option<&RipJob> {
        let selected = self.selected_job.as_ref()?;
        self.jobs.iter().find(|job| &job.id == selected)
    }

    pub(crate) fn selected_job_mut(&mut self) -> Option<&mut RipJob> {
        let selected = self.selected_job.as_ref()?;
        self.jobs.iter_mut().find(|job| &job.id == selected)
    }

    pub(crate) fn job_mut(&mut self, id: &JobId) -> Option<&mut RipJob> {
        self.jobs.iter_mut().find(|job| &job.id == id)
    }

    pub(crate) fn refresh_output_path(&mut self) {
        let Some(job) = self.selected_job() else {
            return;
        };
        if matches!(job.status, JobStatus::Ripping | JobStatus::Completed) {
            return;
        }

        let output = self
            .output_settings
            .output_path(Path::new(&job.input))
            .to_string_lossy()
            .into_owned();
        if let Some(job) = self.selected_job_mut() {
            job.output = output;
        }
    }

    pub(crate) fn can_start(&self) -> bool {
        matches!(self.dependency_state, DependencyState::Ready(_))
            && matches!(
                self.selected_job().map(|job| &job.status),
                Some(JobStatus::Ready)
            )
            && self.output_settings.has_folder()
    }

    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self.selected_job().map(|job| &job.status),
            Some(JobStatus::Probing | JobStatus::Ripping)
        )
    }
}
