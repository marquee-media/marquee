use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Creates a fingerprint from a file's size and first MiB.
pub fn create(path: &Path) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();

    let mut buffer = Vec::with_capacity(1024 * 1024);
    file.take(1024 * 1024).read_to_end(&mut buffer)?;

    let mut hasher = Sha256::new();
    hasher.update(&buffer);

    let hash = hasher.finalize();
    let hash_hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();

    Ok(format!("{size}:{hash_hex}"))
}

#[cfg(test)]
mod tests {
    use super::create;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestFile {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TestFile {
        fn new(contents: &[u8]) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);

            let directory = std::env::temp_dir().join(format!(
                "marquee-file-key-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));

            fs::create_dir(&directory).unwrap();

            let path = directory.join("sample.mkv");
            fs::write(&path, contents).unwrap();

            Self { directory, path }
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn identical_files_produce_identical_fingerprints() {
        let first = TestFile::new(b"same video content");
        let second = TestFile::new(b"same video content");

        assert_eq!(create(&first.path).unwrap(), create(&second.path).unwrap());
    }

    #[test]
    fn different_file_sizes_produce_different_fingerprints() {
        let first = TestFile::new(b"video");
        let second = TestFile::new(b"video content");

        assert_ne!(create(&first.path).unwrap(), create(&second.path).unwrap());
    }

    #[test]
    fn different_contents_within_first_mib_produce_different_fingerprints() {
        let first = TestFile::new(b"video content A");
        let second = TestFile::new(b"video content B");

        assert_ne!(create(&first.path).unwrap(), create(&second.path).unwrap());
    }
}
