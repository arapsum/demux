#[cfg(any(not(debug_assertions), test))]
use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
#[cfg(any(not(debug_assertions), test))]
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

const DEFAULT_FILTER: &str = "demux=info";
#[cfg(not(debug_assertions))]
const LOG_DIRECTORY_OVERRIDE: &str = "DEMUX_LOG_DIR";
#[cfg(any(not(debug_assertions), test))]
const LOG_FILE_PREFIX: &str = "demux";
#[cfg(any(not(debug_assertions), test))]
const MAX_LOG_FILES: usize = 7;
#[cfg(not(debug_assertions))]
const BUFFERED_LINES_LIMIT: usize = 1_024;

/// Owns resources needed to flush Demux's tracing output during shutdown.
///
/// The value must remain alive for the entire application lifetime. In release
/// builds it owns the worker guard for the rolling file appender; in debug
/// builds it is an empty guard for stderr output.
#[derive(Default)]
pub struct TelemetryGuard {
    _worker_guard: Option<WorkerGuard>,
}

/// Initializes Demux's structured tracing subscriber.
///
/// Debug builds write formatted events to stderr. Release builds write
/// human-readable, daily-rotating files to the platform log directory. The
/// `RUST_LOG` environment variable overrides the default `demux=info` filter.
/// If a release log file cannot be initialized, initialization reports the
/// failure to stderr and falls back to the stderr subscriber so the
/// application can still start.
///
/// # Returns
///
/// A guard that must be held until after the application's final tracing event
/// so buffered file events can be flushed.
#[must_use]
pub fn init() -> TelemetryGuard {
    #[cfg(debug_assertions)]
    {
        init_stderr();
        TelemetryGuard::default()
    }

    #[cfg(not(debug_assertions))]
    {
        match init_file() {
            Ok(guard) => guard,
            Err(error) => {
                eprintln!("could not initialize production logging: {error}");
                init_stderr();
                TelemetryGuard::default()
            }
        }
    }
}

fn init_stderr() {
    let result = tracing_subscriber::fmt()
        .with_env_filter(filter())
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init();

    if let Err(error) = result {
        eprintln!("could not install tracing subscriber: {error}");
    }
}

#[cfg(not(debug_assertions))]
fn init_file() -> Result<TelemetryGuard, TelemetryError> {
    let directory = log_directory()?;
    std::fs::create_dir_all(&directory).map_err(|source| TelemetryError::Directory {
        path: directory.clone(),
        source,
    })?;

    let appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(&directory)
        .map_err(|source| TelemetryError::Appender {
            path: directory.clone(),
            source,
        })?;
    let (writer, worker_guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(BUFFERED_LINES_LIMIT)
        .lossy(true)
        .thread_name("demux-log-writer")
        .finish(appender);

    if let Err(error) = tracing_subscriber::fmt()
        .with_env_filter(filter())
        .with_target(false)
        .with_ansi(false)
        .with_writer(writer)
        .try_init()
    {
        drop(worker_guard);
        return Err(TelemetryError::Subscriber(error.to_string()));
    }

    tracing::info!(directory = %directory.display(), "production file logging initialized");

    Ok(TelemetryGuard {
        _worker_guard: Some(worker_guard),
    })
}

fn filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
}

#[cfg(not(debug_assertions))]
fn log_directory() -> Result<PathBuf, TelemetryError> {
    let override_directory = std::env::var_os(LOG_DIRECTORY_OVERRIDE)
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from);

    #[cfg(target_os = "windows")]
    let platform_directory = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("Demux").join("logs"));

    #[cfg(target_os = "macos")]
    let platform_directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|base| base.join("Library/Logs/Demux"));

    #[cfg(all(unix, not(target_os = "macos")))]
    let platform_directory = std::env::var_os("XDG_STATE_HOME")
        .filter(|directory| !directory.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|base| base.join(".local/state"))
        })
        .map(|base| base.join("demux/logs"));

    resolve_log_directory(override_directory, platform_directory)
}

#[cfg(any(not(debug_assertions), test))]
fn resolve_log_directory(
    override_directory: Option<PathBuf>,
    platform_directory: Option<PathBuf>,
) -> Result<PathBuf, TelemetryError> {
    override_directory
        .or(platform_directory)
        .ok_or(TelemetryError::DirectoryUnavailable)
}

#[cfg(any(not(debug_assertions), test))]
#[derive(Debug, thiserror::Error)]
enum TelemetryError {
    #[error("could not determine a platform log directory")]
    DirectoryUnavailable,
    #[cfg(not(debug_assertions))]
    #[error("could not create log directory `{path}`: {source}")]
    Directory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[cfg(not(debug_assertions))]
    #[error("could not create a rolling appender in `{path}`: {source}")]
    Appender {
        path: PathBuf,
        #[source]
        source: rolling::InitError,
    },
    #[cfg(not(debug_assertions))]
    #[error("could not install the file tracing subscriber: {0}")]
    Subscriber(String),
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Read,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temporary_log_directory(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("demux-{name}-{}-{suffix}", std::process::id()))
    }

    #[test]
    fn explicit_log_directory_override_wins() {
        let override_directory = PathBuf::from("/tmp/demux-logs");
        let platform_directory = PathBuf::from("/tmp/platform-logs");

        assert_eq!(
            resolve_log_directory(Some(override_directory.clone()), Some(platform_directory))
                .expect("override should resolve"),
            override_directory
        );
    }

    #[test]
    fn missing_log_directories_are_reported() {
        assert!(matches!(
            resolve_log_directory(None, None),
            Err(TelemetryError::DirectoryUnavailable)
        ));
    }

    #[test]
    fn rolling_writer_emits_readable_events_without_ansi() {
        let directory = temporary_log_directory("telemetry");
        fs::create_dir_all(&directory).expect("temporary log directory should be creatable");
        let appender = rolling::RollingFileAppender::builder()
            .rotation(rolling::Rotation::DAILY)
            .filename_prefix(LOG_FILE_PREFIX)
            .filename_suffix("log")
            .max_log_files(MAX_LOG_FILES)
            .build(&directory)
            .expect("temporary rolling appender should be creatable");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_target(false)
            .with_writer(writer)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("telemetry test event");
        });
        drop(guard);

        let mut contents = String::new();
        let entry = fs::read_dir(&directory)
            .expect("temporary log directory should be readable")
            .next()
            .expect("rolling appender should create one log file")
            .expect("log directory entry should be readable");
        fs::File::open(entry.path())
            .expect("log file should be openable")
            .read_to_string(&mut contents)
            .expect("log file should contain UTF-8 text");

        assert!(contents.contains("telemetry test event"));
        assert!(!contents.contains('\u{1b}'));

        fs::remove_dir_all(directory).expect("temporary log directory should be removable");
    }
}
