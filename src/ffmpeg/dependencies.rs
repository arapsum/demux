use std::{io, process::Command};

use super::error::DependencyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependencies {
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyState {
    Checking,
    Ready(Dependencies),
    Missing {
        program: &'static str,
    },
    Failed {
        program: &'static str,
        message: String,
    },
}

impl From<&DependencyError> for DependencyState {
    fn from(error: &DependencyError) -> Self {
        match error {
            DependencyError::Missing { program } => Self::Missing { program },
            DependencyError::Launch {
                program, source, ..
            } => Self::Failed {
                program,
                message: source.to_string(),
            },
            DependencyError::Failed {
                program,
                status,
                stderr,
            } => Self::Failed {
                program,
                message: format!("{status}: {}", stderr.trim()),
            },
        }
    }
}

pub fn detect_dependencies() -> Result<Dependencies, DependencyError> {
    Ok(Dependencies {
        ffmpeg_version: detect_program("ffmpeg")?,
        ffprobe_version: detect_program("ffprobe")?,
    })
}

fn detect_program(program: &'static str) -> Result<String, DependencyError> {
    let output = Command::new(program)
        .arg("-version")
        .output()
        .map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => DependencyError::Missing { program },
            _ => DependencyError::Launch { program, source },
        })?;

    if !output.status.success() {
        return Err(DependencyError::Failed {
            program,
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::to_owned)
        .unwrap_or_else(|| program.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_program_has_an_actionable_message() {
        let message = DependencyError::Missing { program: "ffmpeg" }.to_string();

        assert!(message.contains("ffmpeg was not found on PATH"));
    }

    #[test]
    fn missing_program_maps_to_missing_state() {
        let error = DependencyError::Missing { program: "ffprobe" };

        assert_eq!(
            DependencyState::from(&error),
            DependencyState::Missing { program: "ffprobe" }
        );
    }

    #[test]
    fn missing_executable_is_detected() {
        const MISSING_PROGRAM: &str = "demux-command-that-does-not-exist";

        let error = detect_program(MISSING_PROGRAM).unwrap_err();

        assert!(matches!(
            error,
            DependencyError::Missing {
                program: MISSING_PROGRAM
            }
        ));
    }

    #[test]
    fn launch_error_preserves_the_failing_program() {
        let error = DependencyError::Launch {
            program: "ffprobe",
            source: io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
        };

        assert_eq!(
            DependencyState::from(&error),
            DependencyState::Failed {
                program: "ffprobe",
                message: "permission denied".to_owned(),
            }
        );
    }
}
