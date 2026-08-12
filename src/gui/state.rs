use std::path::{Path, PathBuf};

use crate::{
    ffmpeg::DependencyState,
    model::job::{JobId, JobStatus, RipJob},
};

#[derive(Debug)]
pub struct Demux {
    pub(crate) dependency_state: DependencyState,
    pub(crate) jobs: Vec<RipJob>,
    pub(crate) selected_job: Option<JobId>,
    pub(crate) output_folder: String,
    pub(crate) error: Option<String>,
    pub(crate) picking_file: bool,
    next_job_id: u64,
}

impl Default for Demux {
    fn default() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            jobs: Vec::new(),
            selected_job: None,
            output_folder: String::new(),
            error: None,
            picking_file: false,
            next_job_id: 1,
        }
    }
}

impl Demux {
    pub(crate) fn add_job(&mut self, input: PathBuf) -> JobId {
        let id = JobId::new(self.next_job_id);
        self.next_job_id += 1;

        if self.output_folder.is_empty()
            && let Some(parent) = input.parent()
        {
            self.output_folder = parent.to_string_lossy().into_owned();
        }

        let output = output_path(&input, &self.output_folder);
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
        let output_folder = self.output_folder.clone();
        if let Some(job) = self.selected_job_mut()
            && !matches!(job.status, JobStatus::Ripping | JobStatus::Completed)
        {
            job.output = output_path(Path::new(&job.input), &output_folder)
                .to_string_lossy()
                .into_owned();
        }
    }

    pub(crate) fn can_start(&self) -> bool {
        matches!(self.dependency_state, DependencyState::Ready(_))
            && matches!(
                self.selected_job().map(|job| &job.status),
                Some(JobStatus::Ready)
            )
            && !self.output_folder.trim().is_empty()
    }

    pub(crate) fn is_busy(&self) -> bool {
        matches!(
            self.selected_job().map(|job| &job.status),
            Some(JobStatus::Probing | JobStatus::Ripping)
        )
    }
}

fn output_path(input: &Path, output_folder: &str) -> PathBuf {
    let filename = input
        .file_name()
        .map_or_else(|| PathBuf::from("output"), PathBuf::from)
        .with_extension("mp3");

    if output_folder.trim().is_empty() {
        input
            .parent()
            .map_or_else(PathBuf::new, PathBuf::from)
            .join(filename)
    } else {
        Path::new(output_folder.trim()).join(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_file_defaults_output_to_its_directory() {
        let mut state = Demux::default();

        state.add_job(PathBuf::from("/videos/example.mov"));

        assert_eq!(state.output_folder, "/videos");
        assert_eq!(
            state.selected_job().map(|job| job.output.as_str()),
            Some("/videos/example.mp3")
        );
    }

    #[test]
    fn changing_output_folder_refreshes_pending_job_path() {
        let mut state = Demux::default();
        state.add_job(PathBuf::from("/videos/example.mov"));

        state.output_folder = "/music".into();
        state.refresh_output_path();

        assert_eq!(
            state.selected_job().map(|job| job.output.as_str()),
            Some("/music/example.mp3")
        );
    }
}
