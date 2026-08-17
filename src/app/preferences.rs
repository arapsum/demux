use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};

use crate::{
    Error, Result,
    model::{encoding::RipOptions, source::DestinationPolicy},
};

const PREFERENCES_FILENAME: &str = "settings.json";
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
static LATEST_REVISION: AtomicU64 = AtomicU64::new(0);
static WRITER: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    version: u8,
    encoding: RipOptions,
    destination: DestinationPolicy,
    window: WindowPreferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferenceDefaults {
    pub encoding: RipOptions,
    pub destination: DestinationPolicy,
    pub window: WindowPreferences,
}

/// Window behavior and the last usable desktop geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowPreferences {
    pub remember_geometry: bool,
    pub geometry: Option<WindowGeometry>,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            remember_geometry: true,
            geometry: None,
        }
    }
}

/// A serialized window position and client-area size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

/// Loads the persisted application defaults.
///
/// # Returns
///
/// The validated encoding, destination, and window defaults, or built-in
/// defaults when no preference file exists.
///
/// # Errors
///
/// Returns an error when:
///
/// - The preferences directory cannot be determined.
/// - The preferences file cannot be read.
/// - The stored JSON cannot be parsed.
pub async fn load() -> Result<PreferenceDefaults> {
    load_from(&preferences_path()?).await
}

pub fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, Ordering::Relaxed)
}

/// Persists the application defaults when the revision is current.
///
/// # Parameters
///
/// - `encoding`: Encoding defaults to save.
/// - `destination`: Destination policy to save.
/// - `window`: Window behavior and geometry to save.
/// - `revision`: Monotonic revision identifying this save request.
///
/// # Returns
///
/// `Ok(())` after the latest requested defaults are persisted, or when a
/// newer request has superseded this revision.
///
/// # Errors
///
/// Returns an error when:
///
/// - The preferences directory cannot be determined.
/// - The preferences directory cannot be created.
/// - The defaults cannot be serialized as JSON.
/// - The preferences file cannot be written.
pub async fn save(
    encoding: RipOptions,
    destination: DestinationPolicy,
    window: WindowPreferences,
    revision: u64,
) -> Result<()> {
    let _guard = WRITER
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    if revision < LATEST_REVISION.load(Ordering::Acquire) {
        return Ok(());
    }
    LATEST_REVISION.store(revision, Ordering::Release);
    save_to(&preferences_path()?, encoding, destination, window).await
}

/// Loads defaults from one explicit preferences file.
///
/// # Parameters
///
/// - `path`: Location of the preferences file.
///
/// # Returns
///
/// The persisted defaults, or built-in defaults when the file does not exist.
///
/// # Errors
///
/// Returns an error when:
///
/// - The file cannot be read for a reason other than it being absent.
/// - The file contains invalid preference JSON.
async fn load_from(path: &Path) -> Result<PreferenceDefaults> {
    let contents = match tokio::fs::read(path).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PreferenceDefaults {
                encoding: RipOptions::default(),
                destination: DestinationPolicy::default(),
                window: WindowPreferences::default(),
            });
        }
        Err(source) => {
            return Err(Error::PreferencesRead {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let preferences: Preferences =
        serde_json::from_slice(&contents).map_err(|source| Error::PreferencesParse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(PreferenceDefaults {
        encoding: preferences.encoding,
        destination: preferences.destination,
        window: preferences.window,
    })
}

/// Serializes and writes defaults to one explicit preferences file.
///
/// # Parameters
///
/// - `path`: Destination preferences file.
/// - `encoding`: Encoding defaults to serialize.
/// - `destination`: Destination policy to serialize.
/// - `window`: Window behavior and geometry to serialize.
///
/// # Returns
///
/// `Ok(())` after the preferences file is written.
///
/// # Errors
///
/// Returns an error when:
///
/// - The file has no parent directory.
/// - The parent directory cannot be created.
/// - The preferences cannot be serialized as JSON.
/// - The preferences file cannot be written.
async fn save_to(
    path: &Path,
    encoding: RipOptions,
    destination: DestinationPolicy,
    window: WindowPreferences,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::PreferencesDirectoryUnavailable)?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|source| Error::PreferencesWrite {
            path: path.to_path_buf(),
            source,
        })?;
    let contents = serde_json::to_vec_pretty(&Preferences {
        version: 3,
        encoding,
        destination,
        window,
    })?;
    tokio::fs::write(path, contents)
        .await
        .map_err(|source| Error::PreferencesWrite {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

/// Resolves the platform-specific preferences file location.
///
/// # Returns
///
/// The configuration path overridden by `DEMUX_CONFIG_DIR`, when set, or the
/// standard platform configuration path.
///
/// # Errors
///
/// Returns an error when:
///
/// - No supported platform configuration directory can be determined.
fn preferences_path() -> Result<PathBuf> {
    if let Some(directory) = std::env::var_os("DEMUX_CONFIG_DIR") {
        return Ok(PathBuf::from(directory).join(PREFERENCES_FILENAME));
    }

    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support"));

    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".config"))
        });

    base.map(|base| base.join("demux").join(PREFERENCES_FILENAME))
        .ok_or(Error::PreferencesDirectoryUnavailable)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::ffmpeg::{ChannelMode, Mp3Bitrate, SampleRate};

    use super::*;

    fn temporary_settings_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("demux-{name}-{suffix}/settings.json"))
    }

    #[tokio::test]
    async fn missing_preferences_use_valid_defaults() {
        let path = temporary_settings_path("missing-preferences");

        assert_eq!(
            load_from(&path).await.unwrap(),
            PreferenceDefaults {
                encoding: RipOptions::default(),
                destination: DestinationPolicy::default(),
                window: WindowPreferences::default(),
            }
        );
    }

    #[tokio::test]
    async fn saved_encoding_defaults_survive_a_reload() {
        let path = temporary_settings_path("saved-preferences");
        let options = RipOptions {
            bitrate: Mp3Bitrate::Kbps320,
            sample_rate: SampleRate::Hz48000,
            channels: ChannelMode::Mono,
            ..RipOptions::default()
        };

        save_to(
            &path,
            options,
            DestinationPolicy::default(),
            WindowPreferences::default(),
        )
        .await
        .unwrap();
        assert_eq!(load_from(&path).await.unwrap().encoding, options);

        tokio::fs::remove_dir_all(path.parent().unwrap())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn malformed_preferences_are_reported_instead_of_silently_replaced() {
        let path = temporary_settings_path("invalid-preferences");
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, b"{not json").await.unwrap();

        assert!(matches!(
            load_from(&path).await.unwrap_err(),
            Error::PreferencesParse { path: failed, .. } if failed == path
        ));

        tokio::fs::remove_dir_all(path.parent().unwrap())
            .await
            .unwrap();
    }
}
