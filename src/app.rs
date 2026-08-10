use crate::{
    Result,
    ffmpeg::{self, DependencyState},
};

#[derive(Debug)]
pub struct App {
    dependency_state: DependencyState,
}

impl App {
    pub fn new() -> Self {
        Self {
            dependency_state: DependencyState::Checking,
        }
    }

    pub fn dependency_state(&self) -> &DependencyState {
        &self.dependency_state
    }

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

    pub fn run(&mut self) -> Result<()> {
        self.detect_dependencies()?;

        println!("==== Input video to be ripped ====");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        println!("==== Output audio file ====");
        let mut output = String::new();
        std::io::stdin().read_line(&mut output).unwrap();
        let output = output.trim();

        println!("==== Ripping audio... ====");
        if let Ok(cmd_output) = ffmpeg::rip(input, output) {
            println!("Audio ripped successfully: {}", output);
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
