use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The folder selected by the user and the path of a discovered media file
/// relative to that folder.
///
/// Keeping this provenance alongside a job lets the destination resolver
/// preserve a folder import without ever treating an untrusted path as an
/// absolute destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHierarchy {
    root: PathBuf,
    relative_path: PathBuf,
}

impl SourceHierarchy {
    /// Creates a validated source-folder and relative-file pairing.
    ///
    /// # Parameters
    ///
    /// - `root`: Absolute folder selected for the import.
    /// - `relative_path`: Safe path from `root` to the media file.
    ///
    /// # Returns
    ///
    /// A hierarchy record suitable for destination resolution.
    ///
    /// # Errors
    ///
    /// Returns an error when:
    ///
    /// - `root` is not absolute.
    /// - The relative path is empty or absolute.
    /// - The relative path contains parent or current-directory components.
    pub fn new(
        root: impl Into<PathBuf>,
        relative_path: impl Into<PathBuf>,
    ) -> Result<Self, SourceHierarchyError> {
        let root = root.into();
        let relative_path = relative_path.into();
        if !root.is_absolute() {
            return Err(SourceHierarchyError::RootNotAbsolute(root));
        }
        if relative_path.as_os_str().is_empty() || relative_path.is_absolute() {
            return Err(SourceHierarchyError::RelativePathNotRelative(relative_path));
        }
        if relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(SourceHierarchyError::UnsafeRelativePath(relative_path));
        }
        Ok(Self {
            root,
            relative_path,
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    #[must_use]
    pub fn relative_parent(&self) -> Option<&Path> {
        self.relative_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
    }
}

/// Invalid source-folder provenance rejected during intake.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceHierarchyError {
    #[error("source folder is not absolute: {0}")]
    RootNotAbsolute(PathBuf),
    #[error("source path must be relative: {0}")]
    RelativePathNotRelative(PathBuf),
    #[error("source path contains unsafe components: {0}")]
    UnsafeRelativePath(PathBuf),
}

const fn default_enabled() -> bool {
    true
}

/// Destination behavior captured for every job at enqueue time.
/// Destination behavior captured with each queued extraction job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DestinationPolicy {
    #[serde(default = "default_enabled")]
    pub preserve_folder_structure: bool,
}

impl Default for DestinationPolicy {
    fn default() -> Self {
        Self {
            preserve_folder_structure: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_rejects_absolute_and_parent_escaping_paths() {
        assert!(matches!(
            SourceHierarchy::new("/videos", "/episode.mp4"),
            Err(SourceHierarchyError::RelativePathNotRelative(_))
        ));
        assert!(matches!(
            SourceHierarchy::new("/videos", "../episode.mp4"),
            Err(SourceHierarchyError::UnsafeRelativePath(_))
        ));
    }

    #[test]
    fn destination_policy_defaults_to_preserving_folder_structure() {
        assert!(DestinationPolicy::default().preserve_folder_structure);
    }
}
