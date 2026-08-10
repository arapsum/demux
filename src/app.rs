use crate::{
    Result,
    ffmpeg::{self, DependencyState},
};

#[derive(Debug)]
pub struct App {
    dependency_state: DependencyState,
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
    /// Prompts for input and output paths, then asks `FFmpeg` to extract an
    /// MP3 audio stream.
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
    /// let mut app = App::new();
    /// app.run()?;
    /// # Ok::<(), demux::Error>(())
    /// ```
    pub fn run(&mut self) -> Result<()> {
        self.detect_dependencies()?;

        println!("==== Input video to be ripped ====");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        println!("==== Output audio file ====");
        let mut output = String::new();
        std::io::stdin().read_line(&mut output)?;
        let output = output.trim();

        println!("==== Ripping audio... ====");
        if let Ok(cmd_output) = ffmpeg::rip(input, output) {
            println!("Audio ripped successfully: {output}");
            println!("{}", cmd_output.status);
        } else {
            println!("Failed to rip audio");
        }
        println!("==== Done! ====");
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
