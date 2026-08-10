use std::{io, process::Command};

use super::error::DependencyError;

/// Version information for the `FFmpeg` tools required by Demux.
///
/// A value of this type confirms that both `ffmpeg` and `ffprobe` were found
/// on `PATH` and exited successfully when queried with `-version`.
///
/// # Fields
///
/// - `ffmpeg_version`: The first line reported by `ffmpeg -version`.
/// - `ffprobe_version`: The first line reported by `ffprobe -version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependencies {
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
}

/// Represents the current result of checking `FFmpeg` dependencies.
///
/// `App` starts in [`Checking`](Self::Checking), transitions to
/// [`Ready`](Self::Ready) when both tools are available, or records the
/// specific failure state when a required executable cannot be used.
///
/// # Variants
///
/// - [`Checking`](Self::Checking): Dependency detection has not completed.
/// - [`Ready`](Self::Ready): `FFmpeg` and `FFprobe` are available.
/// - [`Missing`](Self::Missing): A required executable is absent from `PATH`.
/// - [`Failed`](Self::Failed): A required executable could not be started or
///   exited unsuccessfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyState {
    /// Dependency detection is pending or currently in progress.
    Checking,
    /// Both required executables are available, including their version
    /// descriptions.
    Ready(Dependencies),
    /// A required executable was not found on `PATH`.
    Missing {
        /// The missing executable name.
        program: &'static str,
    },
    /// A required executable could not be started or exited with an error.
    Failed {
        /// The executable that failed.
        program: &'static str,
        /// A human-readable explanation of the failure.
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

/// Detects the `FFmpeg` tools required by Demux.
///
/// Runs `ffmpeg -version` and `ffprobe -version`, then records the first line
/// printed by each command as its version description.
///
/// # Returns
///
/// The detected `FFmpeg` and `FFprobe` version descriptions.
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
/// use demux::ffmpeg::detect_dependencies;
///
/// let dependencies = detect_dependencies()?;
/// println!("{}", dependencies.ffmpeg_version);
/// println!("{}", dependencies.ffprobe_version);
/// # Ok::<(), demux::Error>(())
/// ```
pub fn detect_dependencies() -> Result<Dependencies, DependencyError> {
    Ok(Dependencies {
        ffmpeg_version: detect_program("ffmpeg")?,
        ffprobe_version: detect_program("ffprobe")?,
    })
}

/// Detects one `FFmpeg` executable and reads its version description.
///
/// The executable is started with the `-version` argument. Only the first
/// line of standard output is retained.
///
/// # Parameters
///
/// - `program`: The executable name to resolve from `PATH`, such as `ffmpeg`
///   or `ffprobe`.
///
/// # Returns
///
/// The first line printed by the program, or the program name when it exits
/// successfully without standard output.
///
/// # Errors
///
/// Returns an error when the executable cannot be checked:
///
/// - The executable is missing from `PATH`.
/// - The operating system cannot start the executable.
/// - The executable exits with a non-success status.
///
/// # Examples
///
/// `detect_dependencies` calls this function for both required executables:
///
/// ```text
/// detect_program("ffmpeg")
/// detect_program("ffprobe")
/// ```
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
