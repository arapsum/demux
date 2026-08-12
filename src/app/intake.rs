use std::{collections::HashSet, path::PathBuf};

pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["mp4", "mkv", "mov", "avi", "wmv", "flv", "mpeg", "mpg"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedInput {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntakeResult {
    pub accepted: Vec<AcceptedInput>,
    pub rejected: Vec<RejectedInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedInput {
    pub path: PathBuf,
    pub size: u64,
}

pub async fn discover(inputs: Vec<PathBuf>, existing: Vec<PathBuf>) -> IntakeResult {
    let mut result = IntakeResult::default();
    let mut seen: HashSet<PathBuf> = existing.into_iter().collect();
    let mut pending: Vec<_> = inputs.into_iter().rev().collect();

    while let Some(path) = pending.pop() {
        let metadata = match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                result.rejected.push(RejectedInput {
                    path,
                    reason: format!("Could not read this path: {error}"),
                });
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            result.rejected.push(RejectedInput {
                path,
                reason: "Symbolic links are not followed".into(),
            });
            continue;
        }

        if metadata.is_dir() {
            let mut directory = match tokio::fs::read_dir(&path).await {
                Ok(directory) => directory,
                Err(error) => {
                    result.rejected.push(RejectedInput {
                        path,
                        reason: format!("Could not read this folder: {error}"),
                    });
                    continue;
                }
            };
            let mut children = Vec::new();
            loop {
                match directory.next_entry().await {
                    Ok(Some(entry)) => children.push(entry.path()),
                    Ok(None) => break,
                    Err(error) => {
                        result.rejected.push(RejectedInput {
                            path: path.clone(),
                            reason: format!("Could not finish reading this folder: {error}"),
                        });
                        break;
                    }
                }
            }
            children.sort();
            pending.extend(children.into_iter().rev());
            continue;
        }

        if !metadata.is_file() || !is_supported(&path) {
            result.rejected.push(RejectedInput {
                path,
                reason: "Unsupported media type".into(),
            });
            continue;
        }

        match tokio::fs::canonicalize(&path).await {
            Ok(canonical) if seen.insert(canonical.clone()) => {
                result.accepted.push(AcceptedInput {
                    path: canonical,
                    size: metadata.len(),
                });
            }
            Ok(_) => result.rejected.push(RejectedInput {
                path,
                reason: "This file is already in the queue".into(),
            }),
            Err(error) => result.rejected.push(RejectedInput {
                path,
                reason: format!("Could not resolve this file: {error}"),
            }),
        }
    }

    result
}

#[must_use]
pub fn is_supported(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn supported_extension_policy_is_case_insensitive() {
        assert!(is_supported(std::path::Path::new("concert.MKV")));
        assert!(!is_supported(std::path::Path::new("notes.txt")));
    }

    #[tokio::test]
    async fn recursively_discovers_supported_files_in_stable_order() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("demux-intake-{suffix}"));
        let nested = root.join("nested");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::write(root.join("b.mkv"), b"video")
            .await
            .unwrap();
        tokio::fs::write(root.join("a.mp4"), b"video")
            .await
            .unwrap();
        tokio::fs::write(nested.join("c.mov"), b"video")
            .await
            .unwrap();
        tokio::fs::write(root.join("notes.txt"), b"text")
            .await
            .unwrap();

        let result = discover(vec![root.clone()], Vec::new()).await;
        let names: Vec<_> = result
            .accepted
            .iter()
            .map(|input| {
                input
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        assert_eq!(names, ["a.mp4", "b.mkv", "c.mov"]);
        assert_eq!(result.rejected.len(), 1);
        assert_eq!(result.accepted[0].size, 5);
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn rejects_a_canonical_path_already_in_the_queue() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("demux-dedup-{suffix}"));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let input = root.join("video.mp4");
        tokio::fs::write(&input, b"video").await.unwrap();
        let canonical = tokio::fs::canonicalize(&input).await.unwrap();

        let result = discover(vec![input], vec![canonical]).await;

        assert!(result.accepted.is_empty());
        assert_eq!(
            result.rejected[0].reason,
            "This file is already in the queue"
        );
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_a_directory_symlink_instead_of_following_a_cycle() {
        use std::os::unix::fs::symlink;

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("demux-symlink-{suffix}"));
        tokio::fs::create_dir_all(&root).await.unwrap();
        symlink(&root, root.join("cycle")).unwrap();

        let result = discover(vec![root.clone()], Vec::new()).await;

        assert!(result.accepted.is_empty());
        assert_eq!(result.rejected.len(), 1);
        assert_eq!(result.rejected[0].reason, "Symbolic links are not followed");
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
