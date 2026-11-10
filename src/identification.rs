use std::path::{Path, PathBuf};

use crate::filename::{FilenameCandidate, MediaKind, parse_filename};
use crate::matching::{MatchResult, TmdbCandidate, TmdbMediaType, match_results};
use crate::scanner::{ScanError, scan_directory};
use crate::tmdb::{TmdbClient, TmdbError};

/// A file that Marquee has scanned and prepared for identification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentificationCandidate {
    pub path: PathBuf,
    pub parsed: FilenameCandidate,
}

/// The result of preparing a directory for media identification.
#[derive(Debug)]
pub struct IdentificationResult {
    pub candidates: Vec<IdentificationCandidate>,
    pub errors: Vec<ScanError>,
}

/// The outcome of attempting to identify one file.
#[derive(Debug)]
pub struct IdentificationOutcome {
    pub candidate: IdentificationCandidate,
    pub status: IdentificationStatus,
}

/// The status of an identification attempt.
#[derive(Debug)]
pub enum IdentificationStatus {
    /// The filename parser was uncertain, so automatic identification
    /// was skipped.
    ManualReview,

    /// TMDb results were successfully evaluated.
    Matched(MatchResult),

    /// The TMDb request failed.
    SearchFailed(TmdbError),
}

/// Video file extensions supported by Marquee.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "mov", "m4v", "mpg", "mpeg", "ts", "m2ts", "webm", "wmv", "flv", "ogv",
    "vob", "3gp",
];

/// Checks whether a path has a supported video extension.
fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

/// Creates an identification candidate from a file path.
pub fn create_candidate(path: &Path) -> IdentificationCandidate {
    IdentificationCandidate {
        path: path.to_path_buf(),
        parsed: parse_filename(path),
    }
}

/// Scans a directory and prepares all supported video files
/// for media identification.
pub fn prepare_directory(root: &Path) -> IdentificationResult {
    let scan_result = scan_directory(root);

    let candidates = scan_result
        .files
        .iter()
        .filter(|file| is_video_file(&file.path))
        .map(|file| create_candidate(&file.path))
        .collect();

    IdentificationResult {
        candidates,
        errors: scan_result.errors,
    }
}

/// Searches TMDb for one identification candidate.
///
/// The year is deliberately not passed as an API filter.
/// Matching handles year differences after the search.
fn search_candidate(
    candidate: &IdentificationCandidate,
    client: &TmdbClient,
) -> Result<Vec<TmdbCandidate>, TmdbError> {
    let title = &candidate.parsed.title;
    let mut results = Vec::new();

    match &candidate.parsed.kind {
        MediaKind::Movie => {
            results.extend(
                client
                    .search_movies(title, None)?
                    .into_iter()
                    .map(|result| TmdbCandidate {
                        media_type: TmdbMediaType::Movie,
                        result,
                    }),
            );
        }

        MediaKind::Episode { .. } => {
            results.extend(client.search_tv(title, None)?.into_iter().map(|result| {
                TmdbCandidate {
                    media_type: TmdbMediaType::Tv,
                    result,
                }
            }));
        }

        MediaKind::NumberedItem { .. } | MediaKind::Unknown => {
            results.extend(
                client
                    .search_movies(title, None)?
                    .into_iter()
                    .map(|result| TmdbCandidate {
                        media_type: TmdbMediaType::Movie,
                        result,
                    }),
            );

            results.extend(client.search_tv(title, None)?.into_iter().map(|result| {
                TmdbCandidate {
                    media_type: TmdbMediaType::Tv,
                    result,
                }
            }));
        }
    }

    Ok(results)
}
/// Prevents uncertain filename parses from producing automatic matches
/// when multiple TMDb results are available.
fn require_manual_selection(result: MatchResult) -> MatchResult {
    match result {
        MatchResult::Automatic(candidate) => MatchResult::NeedsSelection(vec![candidate]),
        other => other,
    }
}
/// Evaluates TMDb results and decides whether identification is automatic
/// or requires manual selection.
fn evaluate_results(
    candidate: &IdentificationCandidate,
    results: Vec<TmdbCandidate>,
) -> MatchResult {
    if results.len() == 1 {
        return MatchResult::Automatic(results.into_iter().next().unwrap());
    }

    let result = match_results(&candidate.parsed, results);

    if candidate.parsed.confident {
        result
    } else {
        require_manual_selection(result)
    }
}
/// Attempts to identify a collection of scanned files using TMDb.
///
/// Uncertain filename parses are preserved for manual review.
/// Attempts to identify one scanned file using TMDb.
pub fn identify_candidate(
    candidate: &IdentificationCandidate,
    client: &TmdbClient,
) -> IdentificationOutcome {
    let status = match search_candidate(candidate, client) {
        Ok(results) => IdentificationStatus::Matched(evaluate_results(candidate, results)),
        Err(error) => IdentificationStatus::SearchFailed(error),
    };

    IdentificationOutcome {
        candidate: candidate.clone(),
        status,
    }
}

/// Attempts to identify a collection of scanned files using TMDb.
pub fn identify_candidates(
    candidates: &[IdentificationCandidate],
    client: &TmdbClient,
) -> Vec<IdentificationOutcome> {
    candidates
        .iter()
        .map(|candidate| identify_candidate(candidate, client))
        .collect()
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
                "marquee-identification-test-{}-{timestamp}",
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
    fn creates_candidate_for_movie() {
        let candidate = create_candidate(Path::new("The Matrix 1999 1080p.mkv"));

        assert_eq!(candidate.parsed.title, "The Matrix");
        assert_eq!(candidate.parsed.year, Some(1999));
        assert_eq!(candidate.parsed.kind, MediaKind::Movie);
        assert!(candidate.parsed.confident);
    }

    #[test]
    fn creates_candidate_for_episode() {
        let path = Path::new("Initial D - 5x01 - Encounter.mkv");
        let candidate = create_candidate(path);

        assert_eq!(candidate.parsed.title, "Initial D");
        assert_eq!(
            candidate.parsed.kind,
            MediaKind::Episode {
                season: Some(5),
                episodes: vec![1],
                episode_title: Some("Encounter".to_string()),
            }
        );
    }

    #[test]
    fn preserves_original_file_path() {
        let path = Path::new("Movies/The Matrix/The Matrix 1999.mkv");
        let candidate = create_candidate(path);

        assert_eq!(candidate.path, path);
    }

    #[test]
    fn uncertain_candidates_remain_uncertain() {
        let candidate = create_candidate(Path::new("Pokemon The Movie.mkv"));

        assert_eq!(candidate.parsed.year, None);
        assert_eq!(candidate.parsed.kind, MediaKind::Unknown);
        assert!(!candidate.parsed.confident);
    }

    #[test]
    fn different_files_remain_separate_candidates() {
        let first = create_candidate(Path::new("The Matrix 1999 1080p.mkv"));
        let second = create_candidate(Path::new("The Matrix 1999 4K.mkv"));

        assert_ne!(first.path, second.path);
        assert_ne!(first, second);
    }

    #[test]
    fn identifies_video_files_in_nested_directories() {
        let temp = TestDirectory::new();

        fs::create_dir_all(temp.path.join("Movies")).unwrap();
        fs::write(temp.path.join("Movies/The Matrix 1999.mkv"), b"video").unwrap();
        fs::write(temp.path.join("Movies/Interstellar 2014.mp4"), b"video").unwrap();

        let result = prepare_directory(&temp.path);

        assert_eq!(result.candidates.len(), 2);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn ignores_non_video_files() {
        let temp = TestDirectory::new();

        fs::write(temp.path.join("movie.mkv"), b"video").unwrap();
        fs::write(temp.path.join("subtitles.srt"), b"subtitle").unwrap();
        fs::write(temp.path.join("poster.jpg"), b"image").unwrap();
        fs::write(temp.path.join("notes.txt"), b"text").unwrap();

        let result = prepare_directory(&temp.path);

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].path, temp.path.join("movie.mkv"));
    }

    #[test]
    fn handles_video_extensions_case_insensitively() {
        let temp = TestDirectory::new();

        fs::write(temp.path.join("movie.MKV"), b"video").unwrap();
        fs::write(temp.path.join("movie.Mp4"), b"video").unwrap();

        let result = prepare_directory(&temp.path);

        assert_eq!(result.candidates.len(), 2);
    }

    #[test]
    fn preserves_scanning_errors() {
        let temp = TestDirectory::new();
        let missing = temp.path.join("missing");

        let result = prepare_directory(&missing);

        assert!(result.candidates.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    fn tmdb_candidate(
        media_type: crate::matching::TmdbMediaType,
        title: &str,
        id: u64,
    ) -> TmdbCandidate {
        let mut result = crate::tmdb::TmdbSearchResult {
            id,
            title: None,
            name: None,
            release_date: None,
            first_air_date: None,
            overview: None,
            poster_path: None,
        };

        match media_type {
            crate::matching::TmdbMediaType::Movie => {
                result.title = Some(title.to_string());
                result.release_date = Some("2001-07-06".to_string());
            }
            crate::matching::TmdbMediaType::Tv => {
                result.name = Some(title.to_string());
                result.first_air_date = Some("2001-07-06".to_string());
            }
        }

        TmdbCandidate { media_type, result }
    }

    #[test]
    fn one_tmdb_result_is_automatic_even_with_low_filename_confidence() {
        let mut candidate = create_candidate(Path::new("4Ever Celebi.mkv"));
        candidate.parsed.confident = false;

        let expected = tmdb_candidate(
            crate::matching::TmdbMediaType::Movie,
            "Pokémon 4Ever",
            12600,
        );

        let result = evaluate_results(&candidate, vec![expected.clone()]);

        assert_eq!(result, MatchResult::Automatic(expected));
    }

    #[test]
    fn movie_and_tv_results_require_manual_selection() {
        let candidate = create_candidate(Path::new("Unknown Title.mkv"));

        let movie = tmdb_candidate(crate::matching::TmdbMediaType::Movie, "Unknown Movie", 1);

        let tv = tmdb_candidate(crate::matching::TmdbMediaType::Tv, "Unknown Show", 2);

        let result = evaluate_results(&candidate, vec![movie.clone(), tv.clone()]);

        assert_eq!(result, MatchResult::NeedsSelection(vec![movie, tv]));
    }

    #[test]
    fn zero_tmdb_results_returns_no_match() {
        let candidate = create_candidate(Path::new("Unknown Title.mkv"));

        let result = evaluate_results(&candidate, vec![]);

        assert_eq!(result, MatchResult::NoMatch);
    }

    #[test]
    fn handles_empty_directories() {
        let temp = TestDirectory::new();

        let result = prepare_directory(&temp.path);

        assert!(result.candidates.is_empty());
        assert!(result.errors.is_empty());
    }
}
