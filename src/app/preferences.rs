use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};

use crate::{Error, Result, model::encoding::RipOptions};

const PREFERENCES_FILENAME: &str = "settings.json";
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
static LATEST_REVISION: AtomicU64 = AtomicU64::new(0);
static WRITER: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    version: u8,
    encoding: RipOptions,
}

pub(crate) async fn load() -> Result<RipOptions> {
    load_from(&preferences_path()?).await
}

pub(crate) fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, Ordering::Relaxed)
}

pub(crate) async fn save(encoding: RipOptions, revision: u64) -> Result<()> {
    let _guard = WRITER
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    if revision < LATEST_REVISION.load(Ordering::Acquire) {
        return Ok(());
    }
    LATEST_REVISION.store(revision, Ordering::Release);
    save_to(&preferences_path()?, encoding).await
}

async fn load_from(path: &Path) -> Result<RipOptions> {
    let contents = match tokio::fs::read(path).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RipOptions::default());
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
    Ok(preferences.encoding)
}

async fn save_to(path: &Path, encoding: RipOptions) -> Result<()> {
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
        version: 1,
        encoding,
    })?;
    tokio::fs::write(path, contents)
        .await
        .map_err(|source| Error::PreferencesWrite {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

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

        assert_eq!(load_from(&path).await.unwrap(), RipOptions::default());
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

        save_to(&path, options).await.unwrap();
        assert_eq!(load_from(&path).await.unwrap(), options);

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
