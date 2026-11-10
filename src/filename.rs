use hunch::{Confidence, Property, hunch};
use std::path::Path;

/// The broad kind of media suggested by a filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaKind {
    Movie,
    Episode {
        season: Option<u32>,
        episodes: Vec<u32>,
        episode_title: Option<String>,
    },
    NumberedItem {
        identifier: String,
        episode_title: Option<String>,
    },
    Unknown,
}

/// Information extracted from a filename.
///
/// This is Marquee's own representation. It deliberately does not expose
/// Hunch types to the rest of the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilenameCandidate {
    pub original_name: String,
    pub title: String,
    pub year: Option<u16>,
    pub kind: MediaKind,
    pub confident: bool,
}

/// Parse a media filename using Hunch and convert its results into
/// Marquee's own filename candidate representation.
pub fn parse_filename(path: &Path) -> FilenameCandidate {
    let original_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let stem = Path::new(&original_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let parsed = hunch(&original_name);

    let extracted_title = parsed.title().map(str::to_string);

    let title = extracted_title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| clean_fallback_title(&stem));

    let year = parsed.year().and_then(|value| u16::try_from(value).ok());

    let season = parsed.season().and_then(|value| u32::try_from(value).ok());

    let mut episodes: Vec<u32> = parsed
        .all(Property::Episode)
        .into_iter()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect();

    if episodes.is_empty()
        && let Some(episode) = parsed.episode().and_then(|value| u32::try_from(value).ok())
    {
        episodes.push(episode);
    }

    episodes.sort_unstable();
    episodes.dedup();

    let episode_title = parsed
        .episode_title()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let confident = extracted_title
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty())
        && matches!(parsed.confidence(), Confidence::High | Confidence::Medium);

    // Do not assign a media kind when Hunch's result is uncertain.
    // Keep the extracted title and year, but leave identification unresolved.
    let kind = if !confident {
        MediaKind::Unknown
    } else if parsed.is_movie() {
        MediaKind::Movie
    } else if parsed.is_episode() || season.is_some() || !episodes.is_empty() {
        MediaKind::Episode {
            season,
            episodes,
            episode_title,
        }
    } else if parsed.is_extra() {
        let identifier = parsed.episode_details().unwrap_or("Extra").to_string();

        MediaKind::NumberedItem {
            identifier,
            episode_title,
        }
    } else {
        MediaKind::Unknown
    };

    FilenameCandidate {
        original_name,
        title,
        year,
        kind,
        confident,
    }
}

/// Make a readable fallback title when Hunch does not extract one.
fn clean_fallback_title(value: &str) -> String {
    value
        .replace(['.', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_movie_with_year() {
        let result = parse_filename(Path::new("Example.Movie.1999.1080p.BluRay.x264-GROUP.mkv"));

        assert_eq!(result.title, "Example Movie");
        assert_eq!(result.year, Some(1999));
        assert_eq!(result.kind, MediaKind::Movie);
        assert!(result.confident);
    }

    #[test]
    fn parses_tv_episode() {
        let result = parse_filename(Path::new(
            "Example.Series.S05E03.720p.BluRay.x264-GROUP.mkv",
        ));

        assert_eq!(result.title, "Example Series");
        assert_eq!(
            result.kind,
            MediaKind::Episode {
                season: Some(5),
                episodes: vec![3],
                episode_title: None,
            }
        );
        assert!(result.confident);
    }

    #[test]
    fn preserves_original_filename() {
        let name = "Example.Movie.1999.1080p.BluRay.x264-GROUP.mkv";
        let result = parse_filename(Path::new(name));

        assert_eq!(result.original_name, name);
    }

    #[test]
    fn parses_movie_with_spaces() {
        let result = parse_filename(Path::new("Example Movie 2020 1080p BluRay.mkv"));

        assert_eq!(result.title, "Example Movie");
        assert_eq!(result.year, Some(2020));
        assert_eq!(result.kind, MediaKind::Movie);
    }

    #[test]
    fn keeps_a_fallback_title_when_no_title_is_extracted() {
        let result = parse_filename(Path::new("Unusual_Filename.mkv"));

        assert!(!result.title.is_empty());
    }

    #[test]
    fn handles_a_filename_without_an_extension() {
        let result = parse_filename(Path::new("Example Movie 1999"));

        assert!(!result.original_name.is_empty());
        assert!(!result.title.is_empty());
    }
}
