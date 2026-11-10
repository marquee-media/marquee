use std::path::PathBuf;

/// A group of related media titles, such as the Initial D franchise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Franchise {
    pub name: String,
    pub titles: Vec<MediaTitle>,
}

/// An individual movie, TV series, or special.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTitle {
    pub title: String,
    pub year: Option<u16>,
    pub content: MediaContent,
}

/// The kind of content represented by a media title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Movie,
    TvSeries,
    Special,
}

/// The content belonging to an individual media title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaContent {
    Movie { versions: Vec<MediaVersion> },
    TvSeries { seasons: Vec<Season> },
    Special { episodes: Vec<Episode> },
}

/// A season belonging to a TV series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Season {
    pub number: u32,
    pub episodes: Vec<Episode>,
}

/// An individual episode belonging to a season or special.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Episode {
    /// The episode number displayed in the library.
    pub number: u32,

    /// The original identifier from the source filename, if available.
    /// Examples: "ESx01", "ESx02".
    pub source_identifier: Option<String>,

    pub title: Option<String>,
    pub versions: Vec<MediaVersion>,
}

/// One independently stored video file belonging to a media title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaVersion {
    pub path: PathBuf,
}

impl MediaTitle {
    /// Returns the type of this media title.
    pub fn media_type(&self) -> MediaType {
        match &self.content {
            MediaContent::Movie { .. } => MediaType::Movie,
            MediaContent::TvSeries { .. } => MediaType::TvSeries,
            MediaContent::Special { .. } => MediaType::Special,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn franchise_can_contain_multiple_media_types() {
        let franchise = Franchise {
            name: "Initial D".to_string(),
            titles: vec![
                MediaTitle {
                    title: "Initial D".to_string(),
                    year: None,
                    content: MediaContent::TvSeries {
                        seasons: Vec::new(),
                    },
                },
                MediaTitle {
                    title: "Initial D: Battle Stage".to_string(),
                    year: None,
                    content: MediaContent::Special {
                        episodes: Vec::new(),
                    },
                },
                MediaTitle {
                    title: "Initial D Third Stage".to_string(),
                    year: Some(2001),
                    content: MediaContent::Movie {
                        versions: Vec::new(),
                    },
                },
            ],
        };

        assert_eq!(franchise.name, "Initial D");
        assert_eq!(franchise.titles.len(), 3);

        assert_eq!(franchise.titles[0].media_type(), MediaType::TvSeries);
        assert_eq!(franchise.titles[1].media_type(), MediaType::Special);
        assert_eq!(franchise.titles[2].media_type(), MediaType::Movie);
    }

    #[test]
    fn movie_can_have_multiple_versions() {
        let movie = MediaTitle {
            title: "The Matrix".to_string(),
            year: Some(1999),
            content: MediaContent::Movie {
                versions: vec![
                    MediaVersion {
                        path: PathBuf::from("Movies/The Matrix 1080p.mkv"),
                    },
                    MediaVersion {
                        path: PathBuf::from("Movies/The Matrix 4K.mkv"),
                    },
                ],
            },
        };

        let MediaContent::Movie { versions } = &movie.content else {
            panic!("Expected a movie");
        };

        assert_eq!(versions.len(), 2);
        assert_ne!(versions[0].path, versions[1].path);
    }

    #[test]
    fn tv_series_can_contain_seasons_and_episodes() {
        let series = MediaTitle {
            title: "Initial D".to_string(),
            year: None,
            content: MediaContent::TvSeries {
                seasons: vec![Season {
                    number: 1,
                    episodes: vec![Episode {
                        number: 1,
                        source_identifier: None,
                        title: Some("The Ultimate Tofu Store Drift".to_string()),
                        versions: vec![
                            MediaVersion {
                                path: PathBuf::from("Initial D/First Stage/01.mkv"),
                            },
                            MediaVersion {
                                path: PathBuf::from("Initial D/First Stage/01 1080p.mkv"),
                            },
                        ],
                    }],
                }],
            },
        };

        let MediaContent::TvSeries { seasons } = &series.content else {
            panic!("Expected a TV series");
        };

        assert_eq!(seasons.len(), 1);
        assert_eq!(seasons[0].number, 1);
        assert_eq!(seasons[0].episodes.len(), 1);
        assert_eq!(seasons[0].episodes[0].number, 1);
        assert_eq!(
            seasons[0].episodes[0].title.as_deref(),
            Some("The Ultimate Tofu Store Drift")
        );
        assert_eq!(seasons[0].episodes[0].versions.len(), 2);
    }

    #[test]
    fn special_can_contain_multiple_episodes_with_original_identifiers() {
        let special = MediaTitle {
            title: "Initial D Extra Stage".to_string(),
            year: None,
            content: MediaContent::Special {
                episodes: vec![
                    Episode {
                        number: 1,
                        source_identifier: Some("ESx01".to_string()),
                        title: Some("Beyond The Impact Blue".to_string()),
                        versions: vec![MediaVersion {
                            path: PathBuf::from("Initial D/ESx01.mkv"),
                        }],
                    },
                    Episode {
                        number: 2,
                        source_identifier: Some("ESx02".to_string()),
                        title: Some("Sentimental White".to_string()),
                        versions: vec![MediaVersion {
                            path: PathBuf::from("Initial D/ESx02.mkv"),
                        }],
                    },
                ],
            },
        };

        let MediaContent::Special { episodes } = &special.content else {
            panic!("Expected a special");
        };

        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].number, 1);
        assert_eq!(episodes[0].source_identifier.as_deref(), Some("ESx01"));
        assert_eq!(episodes[1].number, 2);
        assert_eq!(episodes[1].source_identifier.as_deref(), Some("ESx02"));
    }

    #[test]
    fn media_version_preserves_its_path() {
        let version = MediaVersion {
            path: PathBuf::from("Movies/Example Movie.mkv"),
        };

        assert_eq!(version.path, PathBuf::from("Movies/Example Movie.mkv"));
    }
}
