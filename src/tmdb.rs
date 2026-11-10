use std::env;
use std::error::Error;
use std::fmt;

use reqwest::blocking::Client;
use serde::Deserialize;

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";

/// Errors that can occur when communicating with TMDb.
#[derive(Debug)]
pub enum TmdbError {
    MissingApiKey,
    Request(reqwest::Error),
}

impl fmt::Display for TmdbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => {
                write!(f, "TMDB_API_KEY environment variable is not set")
            }
            Self::Request(error) => write!(f, "TMDb request failed: {error}"),
        }
    }
}

impl Error for TmdbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingApiKey => None,
            Self::Request(error) => Some(error),
        }
    }
}

impl From<reqwest::Error> for TmdbError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

/// A client for communicating with The Movie Database.
pub struct TmdbClient {
    client: Client,
    api_key: String,
}

/// A movie or TV series returned by a TMDb search.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct TmdbSearchResult {
    pub id: u64,

    #[serde(default)]
    pub title: Option<String>,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub release_date: Option<String>,

    #[serde(default)]
    pub first_air_date: Option<String>,

    #[serde(default)]
    pub overview: Option<String>,

    #[serde(default)]
    pub poster_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<TmdbSearchResult>,
}

impl TmdbClient {
    /// Creates a TMDb client using the API key from the environment.
    ///
    /// Loads a project-local .env file when available.
    pub fn from_env() -> Result<Self, TmdbError> {
        dotenvy::dotenv().ok();

        let api_key = env::var("TMDB_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .ok_or(TmdbError::MissingApiKey)?;

        let client = Client::builder().build()?;

        Ok(Self { client, api_key })
    }

    /// Searches TMDb for movies.
    pub fn search_movies(
        &self,
        query: &str,
        year: Option<u16>,
    ) -> Result<Vec<TmdbSearchResult>, TmdbError> {
        let url = format!("{TMDB_BASE_URL}/search/movie");

        let mut request = self
            .client
            .get(url)
            .query(&[("api_key", self.api_key.as_str()), ("query", query)]);

        if let Some(year) = year {
            request = request.query(&[("year", year.to_string())]);
        }

        let response: SearchResponse = request.send()?.error_for_status()?.json()?;

        Ok(response.results)
    }

    /// Searches TMDb for TV series.
    pub fn search_tv(
        &self,
        query: &str,
        year: Option<u16>,
    ) -> Result<Vec<TmdbSearchResult>, TmdbError> {
        let url = format!("{TMDB_BASE_URL}/search/tv");

        let mut request = self
            .client
            .get(url)
            .query(&[("api_key", self.api_key.as_str()), ("query", query)]);

        if let Some(year) = year {
            request = request.query(&[("first_air_date_year", year.to_string())]);
        }

        let response: SearchResponse = request.send()?.error_for_status()?.json()?;

        Ok(response.results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_movie_search_result() {
        let json = r#"
        {
        "id": 157336,
        "title": "Interstellar",
        "release_date": "2014-11-05",
        "overview": "A test overview",
        "poster_path": "/example.jpg"
    }
    "#;

        let result: TmdbSearchResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.id, 157336);
        assert_eq!(result.title.as_deref(), Some("Interstellar"));
        assert_eq!(result.release_date.as_deref(), Some("2014-11-05"));
    }

    #[test]
    fn deserializes_tv_search_result() {
        let json = r#"
        {
        "id": 1396,
        "name": "Breaking Bad",
        "first_air_date": "2008-01-20"
    }
    "#;

        let result: TmdbSearchResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.id, 1396);
        assert_eq!(result.name.as_deref(), Some("Breaking Bad"));
        assert_eq!(result.first_air_date.as_deref(), Some("2008-01-20"));
    }
}
