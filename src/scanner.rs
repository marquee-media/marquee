use std::fs;
use std::path::{Path, PathBuf};

/// Information about a discovered file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    pub name: String,
    pub extension: Option<String>,
    pub path: PathBuf,
}

/// An error encountered while scanning.
#[derive(Debug)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

/// Results of a directory scan.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub files: Vec<ScannedFile>,
    pub errors: Vec<ScanError>,
}

/// Recursively scans a directory without modifying its contents.
///
/// Symbolic links are ignored. Errors are recorded so scanning
/// can continue wherever possible.
pub fn scan_directory(root: &Path) -> ScanResult {
    let mut result = ScanResult::default();

    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            scan_recursive(root, &mut result);
        }
        Ok(_) => {
            result.errors.push(ScanError {
                path: root.to_path_buf(),
                message: "The provided path is not a directory.".to_string(),
            });
        }
        Err(error) => {
            result.errors.push(ScanError {
                path: root.to_path_buf(),
                message: error.to_string(),
            });
        }
    }

    result.files.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

fn scan_recursive(directory: &Path, result: &mut ScanResult) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            result.errors.push(ScanError {
                path: directory.to_path_buf(),
                message: error.to_string(),
            });
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.errors.push(ScanError {
                    path: directory.to_path_buf(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        let path = entry.path();

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                result.errors.push(ScanError {
                    path,
                    message: error.to_string(),
                });
                continue;
            }
        };

        let file_type = metadata.file_type();

        // Never follow symbolic links.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            scan_recursive(&path, result);
        } else if file_type.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();

            let extension = path
                .extension()
                .map(|extension| extension.to_string_lossy().into_owned());

            result.files.push(ScannedFile {
                name,
                extension,
                path,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let path = std::env::temp_dir().join(format!(
                "marquee-scanner-test-{}-{timestamp}",
                std::process::id()
            ));

            fs::create_dir_all(&path).unwrap();

            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn finds_files_in_nested_directories() {
        let temp = TestDirectory::new();

        fs::create_dir_all(temp.path.join("Movies/Science Fiction")).unwrap();
        fs::write(temp.path.join("Movies/Interstellar.mkv"), b"test").unwrap();
        fs::write(temp.path.join("Movies/Science Fiction/Dune.mp4"), b"test").unwrap();

        let result = scan_directory(&temp.path);

        assert_eq!(result.files.len(), 2);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn finds_all_file_types() {
        let temp = TestDirectory::new();

        fs::write(temp.path.join("movie.mkv"), b"test").unwrap();
        fs::write(temp.path.join("subtitles.srt"), b"test").unwrap();
        fs::write(temp.path.join("notes.txt"), b"test").unwrap();

        let result = scan_directory(&temp.path);

        assert_eq!(result.files.len(), 3);
    }

    #[test]
    fn handles_files_without_extensions() {
        let temp = TestDirectory::new();

        fs::write(temp.path.join("README"), b"test").unwrap();

        let result = scan_directory(&temp.path);

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].extension, None);
    }

    #[test]
    fn handles_empty_directories() {
        let temp = TestDirectory::new();

        let result = scan_directory(&temp.path);

        assert!(result.files.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn reports_missing_directories() {
        let temp = TestDirectory::new();
        let missing = temp.path.join("does-not-exist");

        let result = scan_directory(&missing);

        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn rejects_a_file_as_the_root_directory() {
        let temp = TestDirectory::new();
        let file = temp.path.join("movie.mkv");

        fs::write(&file, b"test").unwrap();

        let result = scan_directory(&file);

        assert!(result.files.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn ignores_file_and_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let scan_root = temp.path.join("library");
        let targets = temp.path.join("targets");

        fs::create_dir_all(&scan_root).unwrap();
        fs::create_dir_all(&targets).unwrap();

        fs::write(scan_root.join("normal.mkv"), b"test").unwrap();
        fs::write(targets.join("linked.mkv"), b"test").unwrap();

        symlink(targets.join("linked.mkv"), scan_root.join("file-link.mkv")).unwrap();

        symlink(&targets, scan_root.join("directory-link")).unwrap();

        let result = scan_directory(&scan_root);

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].name, "normal.mkv");
        assert!(result.errors.is_empty());
    }

    #[test]
    fn sorts_results_by_path() {
        let temp = TestDirectory::new();

        fs::write(temp.path.join("zebra.mkv"), b"test").unwrap();
        fs::write(temp.path.join("alpha.mkv"), b"test").unwrap();
        fs::write(temp.path.join("middle.mkv"), b"test").unwrap();

        let result = scan_directory(&temp.path);

        let names: Vec<&str> = result.files.iter().map(|file| file.name.as_str()).collect();

        assert_eq!(names, vec!["alpha.mkv", "middle.mkv", "zebra.mkv"]);
    }

    #[cfg(unix)]
    #[test]
    fn handles_inaccessible_subdirectories() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TestDirectory::new();
        let blocked = temp.path.join("blocked");

        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("hidden.mkv"), b"test").unwrap();
        fs::write(temp.path.join("accessible.mkv"), b"test").unwrap();

        let mut permissions = fs::metadata(&blocked).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&blocked, permissions).unwrap();

        let result = scan_directory(&temp.path);

        // Restore permissions so the temporary directory can be cleaned up.
        let mut permissions = fs::metadata(&blocked).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&blocked, permissions).unwrap();

        assert!(
            result
                .files
                .iter()
                .any(|file| file.name == "accessible.mkv"),
            "The scanner should continue scanning accessible directories."
        );

        if result.errors.is_empty() {
            // Privileged environments may still read directories with mode 000.
            assert!(
                result.files.iter().any(|file| file.name == "hidden.mkv"),
                "If access was not denied, the nested file should be found."
            );
        } else {
            assert!(
                result.errors.iter().any(|error| error.path == blocked),
                "The inaccessible directory should be reported."
            );
        }
    }
}
