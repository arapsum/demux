mod output;

use crate::{
    Result,
    ffmpeg::{self, DependencyState},
    ffprobe::{self, ProbeResult},
    model::job::{JobId, JobStatus, RipJob},
    model::media::MediaInfo,
};
use output::output_path;
use tokio::task::JoinHandle;

type ProbeTask = JoinHandle<ProbeResult<MediaInfo>>;

/// Coordinates dependency checks and the interactive audio-ripping workflow.
///
/// An application instance owns the latest [`RipJob`] so callers can inspect
/// its metadata, progress, and terminal status after a workflow completes.
///
/// # Fields
///
/// - `dependency_state`: The latest result of checking `ffmpeg` and `ffprobe`.
/// - `current_job`: The most recently started ripping job, when one exists.
/// - `next_job_id`: The identifier assigned to the next created job.
#[derive(Debug)]
pub struct App {
    dependency_state: DependencyState,
    current_job: Option<RipJob>,
    next_job_id: u64,
}

impl App {
    /// Creates an application with dependency detection pending.
    ///
    /// # Returns
    ///
    /// A new [`App`] whose dependency state is
    /// [`Checking`](DependencyState::Checking).
    ///
    /// # Examples
    ///
    /// ```
    /// use demux::{App, ffmpeg::DependencyState};
    ///
    /// let app = App::new();
    /// assert_eq!(app.dependency_state(), &DependencyState::Checking);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
            current_job: None,
            next_job_id: 1,
        }
    }

    /// Returns the most recently recorded `FFmpeg` dependency state.
    ///
    /// # Returns
    ///
    /// A reference to the dependency state recorded by the latest dependency
    /// check.
    ///
    /// # Examples
    ///
    /// ```
    /// use demux::{App, ffmpeg::DependencyState};
    ///
    /// let app = App::new();
    /// assert_eq!(app.dependency_state(), &DependencyState::Checking);
    /// ```
    #[must_use]
    pub fn dependency_state(&self) -> &DependencyState {
        &self.dependency_state
    }

    /// Returns the most recently started ripping job.
    ///
    /// # Returns
    ///
    /// A reference to the current job, or [`None`] when no job has been
    /// started.
    #[must_use]
    pub fn current_job(&self) -> Option<&RipJob> {
        self.current_job.as_ref()
    }

    /// Detects the `FFmpeg` dependencies required to run the application.
    ///
    /// # Returns
    ///
    /// An empty result after storing
    /// [`Ready`](DependencyState::Ready) or a failure state in the
    /// application.
    ///
    /// # Errors
    ///
    /// Returns an error when either required executable cannot be checked:
    ///
    /// - `ffmpeg` or `ffprobe` is missing from `PATH`.
    /// - The operating system cannot start either program.
    /// - Either program exits with a non-success status.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use demux::App;
    ///
    /// let mut app = App::new();
    /// app.detect_dependencies()?;
    /// # Ok::<(), demux::Error>(())
    /// ```
    pub fn detect_dependencies(&mut self) -> Result<()> {
        self.dependency_state = DependencyState::Checking;
        println!("Checking dependencies");

        match ffmpeg::detect_dependencies() {
            Ok(dependencies) => {
                println!("Using {}", dependencies.ffmpeg_version);
                println!("Using {}", dependencies.ffprobe_version);
                self.dependency_state = DependencyState::Ready(dependencies);
                Ok(())
            }
            Err(error) => {
                self.dependency_state = DependencyState::from(&error);
                Err(error.into())
            }
        }
    }

    /// Runs the interactive audio-ripping workflow.
    ///
    /// Prompts for an input path and optional output directory and filename,
    /// then asks `FFmpeg` to extract an MP3 audio stream.
    ///
    /// When either output value is blank, the corresponding part is derived
    /// from the input path. The input directory is used for a blank output
    /// directory, and the input filename is given the MP3 extension when the
    /// output filename is blank.
    ///
    /// # Returns
    ///
    /// An empty result after the workflow completes.
    ///
    /// # Errors
    ///
    /// Returns an error when the workflow cannot continue:
    ///
    /// - `ffmpeg` or `ffprobe` cannot be detected.
    /// - An input or output path cannot be read from standard input.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use demux::App;
    ///
    /// # async fn example() -> demux::Result<()> {
    /// let mut app = App::new();
    /// app.run().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run(&mut self) -> Result<()> {
        self.detect_dependencies()?;

        let input = Self::read_input_path()?;
        let probe_task = Self::start_metadata_probe(input.clone());
        let output = Self::read_output_path(&input)?;
        let mut job = self.create_job(input, output);
        Self::print_output_path(&job);
        Self::apply_probe_result(&mut job, probe_task).await;

        if matches!(&job.status, JobStatus::Failed(_)) {
            self.finish_job(job);
            return Ok(());
        }

        Self::rip_job(&mut job).await;
        self.finish_job(job);
        Ok(())
    }

    fn read_input_path() -> Result<String> {
        println!("==== Input video to be ripped ====");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_owned())
    }

    fn start_metadata_probe(input: String) -> ProbeTask {
        tokio::spawn(async move {
            let output = ffprobe::probe(&input).await?;
            ffprobe::metadata(&output)
        })
    }

    fn read_output_path(input: &str) -> Result<String> {
        println!("==== Output directory (leave blank to use the input directory) ====");
        let mut output_directory = String::new();
        std::io::stdin().read_line(&mut output_directory)?;

        println!("==== Output filename (leave blank to derive it from the input) ====");
        let mut output_filename = String::new();
        std::io::stdin().read_line(&mut output_filename)?;

        Ok(output_path(
            input,
            (!output_directory.trim().is_empty()).then_some(output_directory.trim()),
            (!output_filename.trim().is_empty()).then_some(output_filename.trim()),
        ))
    }

    fn print_output_path(job: &RipJob) {
        println!("==== Output audio file ====");
        println!("{}", job.output);
    }

    async fn apply_probe_result(job: &mut RipJob, probe_task: ProbeTask) {
        match probe_task.await {
            Ok(Ok(metadata)) => {
                println!("==== Probe output ====");
                println!("{metadata:#?}");
                job.record_metadata(metadata);
            }
            Ok(Err(error)) => {
                let message = error.to_string();
                eprintln!("Metadata probing failed: {message}");
                job.fail(message);
            }
            Err(error) => {
                let message = error.to_string();
                eprintln!("Metadata probing task failed: {message}");
                job.fail(message);
            }
        }
    }

    async fn rip_job(job: &mut RipJob) {
        println!("==== Ripping audio... ====");
        job.start_ripping();

        match ffmpeg::rip(&job.input, &job.output).await {
            Ok(output) if output.status.success() => {
                job.complete();
                println!("Audio ripped successfully: {}", job.output);
                println!("{}", output.status);
            }
            Ok(output) => {
                let message = format!("FFmpeg exited with {}", output.status);
                eprintln!("Audio ripping failed: {message}");
                job.fail(message);
            }
            Err(error) => {
                let message = error.to_string();
                eprintln!("Audio ripping failed: {message}");
                job.fail(message);
            }
        }
    }

    fn finish_job(&mut self, job: RipJob) {
        self.current_job = Some(job);
        println!("==== Done! ====");
    }

    fn create_job(&mut self, input: String, output: String) -> RipJob {
        let id = JobId::new(self.next_job_id);
        self.next_job_id += 1;

        let mut job = RipJob::new(id, input, output);
        job.start_probing();
        job
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn creates_a_probing_job_with_empty_progress() {
        let mut app = App::new();
        let job = app.create_job("input.mp4".into(), "output.mp3".into());

        assert_eq!(job.id, JobId::new(1));
        assert_eq!(job.input, "input.mp4");
        assert_eq!(job.output, "output.mp3");
        assert_eq!(job.status, JobStatus::Probing);
        assert!(job.metadata.is_none());
        assert_eq!(job.progress.duration, Duration::ZERO);
        assert!(job.progress.percent.abs() < f64::EPSILON);
    }

    #[test]
    fn assigns_unique_ids_to_jobs() {
        let mut app = App::new();
        let first = app.create_job("first.mp4".into(), "first.mp3".into());
        let second = app.create_job("second.mp4".into(), "second.mp3".into());

        assert_eq!(first.id, JobId::new(1));
        assert_eq!(second.id, JobId::new(2));
    }
}
