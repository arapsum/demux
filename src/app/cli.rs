use crate::{App, Result, ffmpeg::RipRequest};

use super::{
    output::output_path,
    services::{FfprobeMediaProbe, SystemAudioRipper, SystemDependencyChecker},
    workflow::{RipWorkflow, WorkflowEvent, WorkflowReporter, WorkflowStage},
};

/// Terminal adapter for the audio-ripping workflow.
#[derive(Debug, Default)]
pub struct Cli;

impl Cli {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Runs the terminal extraction workflow from interactive input.
    ///
    /// # Parameters
    ///
    /// - `app`: Application state updated with dependency and job results.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the requested job reaches a terminal state.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Dependency detection fails.
    /// - Interactive input cannot be read.
    /// - The workflow reports a failure.
    pub async fn run(&mut self, app: &mut App) -> Result<()> {
        let workflow = RipWorkflow::new(
            SystemDependencyChecker,
            FfprobeMediaProbe,
            SystemAudioRipper::default(),
        );
        workflow.detect_dependencies(app, self)?;

        let input = Self::read_input_path()?;
        let output = Self::read_output_path(&input)?;
        println!("==== Output audio file ====");
        println!("{output}");

        workflow
            .run_job(app, RipRequest::new(input, output), self)
            .await;
        Ok(())
    }

    /// Reads and trims the source-media path from standard input.
    ///
    /// # Returns
    ///
    /// The user-supplied source path without leading or trailing whitespace.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Standard input cannot be read.
    fn read_input_path() -> Result<String> {
        println!("==== Input video to be ripped ====");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_owned())
    }

    /// Reads output overrides and derives the destination path.
    ///
    /// # Parameters
    ///
    /// - `input`: Source path used when the user leaves an output field blank.
    ///
    /// # Returns
    ///
    /// The output path derived from the entered directory and filename.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - Standard input cannot be read.
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
}

impl WorkflowReporter for Cli {
    fn report(&mut self, event: WorkflowEvent) {
        match event {
            WorkflowEvent::CheckingDependencies => println!("Checking dependencies"),
            WorkflowEvent::DependenciesReady(dependencies) => {
                println!("Using {}", dependencies.ffmpeg_version);
                println!("Using {}", dependencies.ffprobe_version);
            }
            WorkflowEvent::MetadataReady(_) => {}
            WorkflowEvent::Ripping => println!("==== Ripping audio... ===="),
            WorkflowEvent::Completed { output, status } => {
                println!("Audio ripped successfully: {output}");
                println!("{status}");
            }
            WorkflowEvent::Failed { stage, message } => match stage {
                WorkflowStage::Probing => eprintln!("Metadata probing failed: {message}"),
                WorkflowStage::Ripping => eprintln!("Audio ripping failed: {message}"),
            },
            WorkflowEvent::Finished => println!("==== Done! ===="),
        }
    }
}
