use crate::filename::FilenameCandidate;
use crate::tmdb::TmdbSearchResult;

/// The type of media represented by a TMDb search result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmdbMediaType {
    Movie,
    Tv,
}

/// A TMDb result together with its media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmdbCandidate {
    pub media_type: TmdbMediaType,
    pub result: TmdbSearchResult,
}

/// The outcome of comparing a filename against TMDb results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult {
    /// A sufficiently clear and unique match.
    Automatic(TmdbCandidate),

    /// Results exist, but the user must choose.
    NeedsSelection(Vec<TmdbCandidate>),

    /// No TMDb results were found.
    NoMatch,
}

/// Compare a filename candidate with TMDb search results.
///
/// Matching is deliberately conservative:
/// - Exact title and exact year: automatic if unique.
/// - Exact title and year difference of one: automatic if unique.
/// - Exact title without a year: automatic only with one result.
/// - Fuzzy or partial title matches: user selection.
/// - Other year differences: user selection.
/// - No results: no match.
///
/// All returned selection candidates are preserved for manual identification.
pub fn match_results(filename: &FilenameCandidate, candidates: Vec<TmdbCandidate>) -> MatchResult {
    if candidates.is_empty() {
        return MatchResult::NoMatch;
    }

    let exact_matches: Vec<TmdbCandidate> = candidates
        .iter()
        .filter(|candidate| {
            let tmdb_title = match candidate.media_type {
                TmdbMediaType::Movie => candidate.result.title.as_deref(),
                TmdbMediaType::Tv => candidate.result.name.as_deref(),
            };

            tmdb_title.is_some_and(|title| title.trim().eq_ignore_ascii_case(filename.title.trim()))
        })
        .cloned()
        .collect();

    // If no exact title matches exist, preserve every search result
    // so the user can choose a fuzzy or partial match manually.
    if exact_matches.is_empty() {
        return MatchResult::NeedsSelection(candidates);
    }

    if let Some(year) = filename.year {
        let matching_years: Vec<TmdbCandidate> = exact_matches
            .iter()
            .filter(|candidate| {
                result_year(candidate).is_some_and(|result_year| result_year == year)
            })
            .cloned()
            .collect();

        if matching_years.len() == 1 {
            return MatchResult::Automatic(matching_years[0].clone());
        }

        if matching_years.len() > 1 {
            return MatchResult::NeedsSelection(matching_years);
        }

        let nearby_years: Vec<TmdbCandidate> = exact_matches
            .iter()
            .filter(|candidate| {
                result_year(candidate).is_some_and(|result_year| result_year.abs_diff(year) == 1)
            })
            .cloned()
            .collect();

        if nearby_years.len() == 1 {
            return MatchResult::Automatic(nearby_years[0].clone());
        }

        if nearby_years.len() > 1 {
            return MatchResult::NeedsSelection(nearby_years);
        }

        // The title matches, but the year does not.
        // Keep the exact-title results available for manual selection.
        MatchResult::NeedsSelection(exact_matches)
    } else if candidates.len() == 1 && exact_matches.len() == 1 {
        MatchResult::Automatic(exact_matches[0].clone())
    } else {
        MatchResult::NeedsSelection(candidates)
    }
}

/// Extract the release year or first-air year from a TMDb result.
fn result_year(candidate: &TmdbCandidate) -> Option<u16> {
    let date = match candidate.media_type {
        TmdbMediaType::Movie => candidate.result.release_date.as_deref(),
        TmdbMediaType::Tv => candidate.result.first_air_date.as_deref(),
    }?;

    date.get(..4)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filename::MediaKind;

    fn filename(title: &str, year: Option<u16>) -> FilenameCandidate {
        FilenameCandidate {
            original_name: "test.mkv".to_string(),
            title: title.to_string(),
            year,
            kind: MediaKind::Unknown,
            confident: true,
        }
    }

    fn candidate(media_type: TmdbMediaType, title: &str, year: &str, id: u64) -> TmdbCandidate {
        let mut result = TmdbSearchResult {
            id,
            title: None,
            name: None,
            release_date: None,
            first_air_date: None,
            overview: None,
            poster_path: None,
        };

        match media_type {
            TmdbMediaType::Movie => {
                result.title = Some(title.to_string());
                result.release_date = Some(year.to_string());
            }
            TmdbMediaType::Tv => {
                result.name = Some(title.to_string());
                result.first_air_date = Some(year.to_string());
            }
        }

        TmdbCandidate { media_type, result }
    }

    #[test]
    fn exact_title_and_year_selects_automatically() {
        let result = match_results(
            &filename("Interstellar", Some(2014)),
            vec![candidate(
                TmdbMediaType::Movie,
                "Interstellar",
                "2014-11-07",
                1,
            )],
        );

        assert!(matches!(result, MatchResult::Automatic(_)));
    }

    #[test]
    fn exact_title_and_year_one_year_off_selects_automatically() {
        let result = match_results(
            &filename("Interstellar", Some(2014)),
            vec![candidate(
                TmdbMediaType::Movie,
                "Interstellar",
                "2015-01-01",
                1,
            )],
        );

        assert!(matches!(result, MatchResult::Automatic(_)));
    }

    #[test]
    fn exact_title_without_year_and_one_result_selects_automatically() {
        let result = match_results(
            &filename("The Matrix", None),
            vec![candidate(
                TmdbMediaType::Movie,
                "The Matrix",
                "1999-03-31",
                1,
            )],
        );

        assert!(matches!(result, MatchResult::Automatic(_)));
    }

    #[test]
    fn exact_title_without_year_and_multiple_results_needs_selection() {
        let result = match_results(
            &filename("The Matrix", None),
            vec![
                candidate(TmdbMediaType::Movie, "The Matrix", "1999-03-31", 1),
                candidate(TmdbMediaType::Movie, "The Matrix", "2003-05-15", 2),
            ],
        );

        assert!(matches!(result, MatchResult::NeedsSelection(results) if results.len() == 2));
    }

    #[test]
    fn fuzzy_title_preserves_search_results_for_selection() {
        let fuzzy = candidate(
            TmdbMediaType::Movie,
            "Interstellar: The Journey",
            "2014-01-01",
            1,
        );

        let result = match_results(&filename("Interstellar", Some(2014)), vec![fuzzy.clone()]);

        assert_eq!(result, MatchResult::NeedsSelection(vec![fuzzy]));
    }

    #[test]
    fn no_results_returns_no_match() {
        let result = match_results(&filename("Unknown", None), vec![]);

        assert_eq!(result, MatchResult::NoMatch);
    }

    #[test]
    fn exact_title_with_different_year_needs_selection() {
        let result = match_results(
            &filename("The Matrix", Some(1999)),
            vec![candidate(
                TmdbMediaType::Movie,
                "The Matrix",
                "2005-05-15",
                1,
            )],
        );

        assert!(matches!(result, MatchResult::NeedsSelection(results) if results.len() == 1));
    }

    #[test]
    fn multiple_exact_title_and_year_matches_need_selection() {
        let result = match_results(
            &filename("The Matrix", Some(1999)),
            vec![
                candidate(TmdbMediaType::Movie, "The Matrix", "1999-03-31", 1),
                candidate(TmdbMediaType::Movie, "The Matrix", "1999-04-01", 2),
            ],
        );

        assert!(matches!(result, MatchResult::NeedsSelection(results) if results.len() == 2));
    }

    #[test]
    fn exact_title_with_additional_fuzzy_result_needs_selection() {
        let exact = candidate(TmdbMediaType::Movie, "The Matrix", "1999-03-31", 1);

        let fuzzy = candidate(TmdbMediaType::Movie, "The Matrix Reloaded", "2003-05-15", 2);

        let result = match_results(&filename("The Matrix", None), vec![exact, fuzzy]);

        assert!(matches!(result, MatchResult::NeedsSelection(results) if results.len() == 2));
    }

    #[test]
    fn tv_results_use_series_name_and_first_air_year() {
        let result = match_results(
            &filename("Breaking Bad", Some(2008)),
            vec![candidate(
                TmdbMediaType::Tv,
                "Breaking Bad",
                "2008-01-20",
                1396,
            )],
        );

        assert!(matches!(
            result,
            MatchResult::Automatic(TmdbCandidate {
                media_type: TmdbMediaType::Tv,
                ..
            })
        ));
    }
}
