use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::matching::TmdbMediaType;

/// A TMDb result retained for manual identification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCandidate {
    pub tmdb_id: i64,
    pub media_type: TmdbMediaType,
    pub title: String,
    pub year: Option<u16>,
    pub poster_url: Option<String>,
}

/// A file waiting for manual identification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingIdentification {
    pub file_key: String,
    pub original_path: String,
    pub original_filename: String,
    pub candidates: Vec<StoredCandidate>,
}

/// A completed identification to persist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentificationRecord {
    pub tmdb_id: i64,
    pub media_type: TmdbMediaType,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub episode_end: Option<u32>,
    pub is_manual: bool,
}

/// Opens a database and initializes its schema.
pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    initialize(&connection)?;
    Ok(connection)
}

/// Opens an in-memory database, primarily useful for tests.
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let connection = Connection::open_in_memory()?;
    initialize(&connection)?;
    Ok(connection)
}

/// Creates the database tables and enables foreign keys.
pub fn initialize(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS identifications (
            file_key TEXT PRIMARY KEY,
            tmdb_id INTEGER NOT NULL,
            media_type TEXT NOT NULL,
            season INTEGER,
            episode INTEGER,
            episode_end INTEGER,
            is_manual INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS pending_identifications (
        file_key TEXT PRIMARY KEY,
        original_path TEXT NOT NULL,
        original_filename TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS candidates (
        id INTEGER PRIMARY KEY,
        file_key TEXT NOT NULL,
        tmdb_id INTEGER NOT NULL,
        media_type TEXT NOT NULL,
        title TEXT NOT NULL,
        year INTEGER,
        poster_url TEXT,
        FOREIGN KEY (file_key)
    REFERENCES pending_identifications(file_key)
    ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_candidates_file_key
    ON candidates(file_key);
    ",
    )
}

/// Saves a file awaiting manual identification and all its TMDb candidates.
///
/// Saving an existing file replaces its previous candidate list.
pub fn save_pending_identification(
    connection: &mut Connection,
    pending: &PendingIdentification,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;

    transaction.execute(
        "
        INSERT INTO pending_identifications (
            file_key,
            original_path,
            original_filename
    )
    VALUES (?1, ?2, ?3)
    ON CONFLICT(file_key) DO UPDATE SET
    original_path = excluded.original_path,
    original_filename = excluded.original_filename
    ",
        params![
            pending.file_key,
            pending.original_path,
            pending.original_filename
        ],
    )?;

    transaction.execute(
        "DELETE FROM candidates WHERE file_key = ?1",
        [&pending.file_key],
    )?;

    for candidate in &pending.candidates {
        transaction.execute(
            "
            INSERT INTO candidates (
                file_key,
                tmdb_id,
                media_type,
                title,
                year,
                poster_url
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
            params![
                pending.file_key,
                candidate.tmdb_id,
                media_type_to_string(candidate.media_type),
                candidate.title,
                candidate.year.map(i64::from),
                candidate.poster_url
            ],
        )?;
    }

    transaction.commit()
}

/// Retrieves one pending identification and its stored candidates.
pub fn get_pending_identification(
    connection: &Connection,
    file_key: &str,
) -> rusqlite::Result<Option<PendingIdentification>> {
    let pending = connection
        .query_row(
            "
        SELECT file_key, original_path, original_filename
        FROM pending_identifications
        WHERE file_key = ?1
        ",
            [file_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;

    let Some((file_key, original_path, original_filename)) = pending else {
        return Ok(None);
    };

    let candidates = load_candidates(connection, &file_key)?;

    Ok(Some(PendingIdentification {
        file_key,
        original_path,
        original_filename,
        candidates,
    }))
}

/// Retrieves an existing identification for a file.
pub fn get_identification(
    connection: &Connection,
    file_key: &str,
) -> rusqlite::Result<Option<IdentificationRecord>> {
    connection
        .query_row(
            "
        SELECT tmdb_id, media_type, season, episode, episode_end, is_manual
        FROM identifications
        WHERE file_key = ?1
        ",
            [file_key],
            |row| {
                let media_type: String = row.get(1)?;

                Ok(IdentificationRecord {
                    tmdb_id: row.get(0)?,
                    media_type: string_to_media_type(&media_type)?,
                    season: row
                        .get::<_, Option<i64>>(2)?
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    episode: row
                        .get::<_, Option<i64>>(3)?
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    episode_end: row
                        .get::<_, Option<i64>>(4)?
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    is_manual: row.get::<_, i64>(5)? != 0,
                })
            },
        )
        .optional()
}

/// Retrieves every file awaiting manual identification.
pub fn list_pending_identifications(
    connection: &Connection,
) -> rusqlite::Result<Vec<PendingIdentification>> {
    let mut statement = connection.prepare(
        "
        SELECT file_key, original_path, original_filename
        FROM pending_identifications
        ORDER BY original_path
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut pending_identifications = Vec::new();

    for row in rows {
        let (file_key, original_path, original_filename) = row?;
        let candidates = load_candidates(connection, &file_key)?;

        pending_identifications.push(PendingIdentification {
            file_key,
            original_path,
            original_filename,
            candidates,
        });
    }

    Ok(pending_identifications)
}

/// Saves a user's identification and removes its pending review entry.
///
/// The associated candidates are deleted automatically through
/// the foreign-key cascade.
pub fn resolve_pending_identification(
    connection: &mut Connection,
    file_key: &str,
    identification: &IdentificationRecord,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;

    transaction.execute(
        "
        INSERT INTO identifications (
            file_key,
            tmdb_id,
            media_type,
            season,
            episode,
            episode_end,
            is_manual
    )
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT(file_key) DO UPDATE SET
    tmdb_id = excluded.tmdb_id,
    media_type = excluded.media_type,
    season = excluded.season,
    episode = excluded.episode,
    episode_end = excluded.episode_end,
    is_manual = excluded.is_manual
    ",
        params![
            file_key,
            identification.tmdb_id,
            media_type_to_string(identification.media_type),
            identification.season.map(i64::from),
            identification.episode.map(i64::from),
            identification.episode_end.map(i64::from),
            i64::from(identification.is_manual)
        ],
    )?;

    transaction.execute(
        "DELETE FROM pending_identifications WHERE file_key = ?1",
        [file_key],
    )?;

    transaction.commit()
}

fn load_candidates(
    connection: &Connection,
    file_key: &str,
) -> rusqlite::Result<Vec<StoredCandidate>> {
    let mut statement = connection.prepare(
        "
        SELECT tmdb_id, media_type, title, year, poster_url
        FROM candidates
        WHERE file_key = ?1
        ORDER BY id
        ",
    )?;

    let rows = statement.query_map([file_key], |row| {
        let media_type: String = row.get(1)?;

        Ok(StoredCandidate {
            tmdb_id: row.get(0)?,
            media_type: string_to_media_type(&media_type)?,
            title: row.get(2)?,
            year: row
                .get::<_, Option<i64>>(3)?
                .map(u16::try_from)
                .transpose()
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            poster_url: row.get(4)?,
        })
    })?;

    rows.collect()
}

fn media_type_to_string(media_type: TmdbMediaType) -> &'static str {
    match media_type {
        TmdbMediaType::Movie => "movie",
        TmdbMediaType::Tv => "tv",
    }
}

fn string_to_media_type(value: &str) -> rusqlite::Result<TmdbMediaType> {
    match value {
        "movie" => Ok(TmdbMediaType::Movie),
        "tv" => Ok(TmdbMediaType::Tv),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_candidate(id: i64, title: &str) -> StoredCandidate {
        StoredCandidate {
            tmdb_id: id,
            media_type: TmdbMediaType::Movie,
            title: title.to_string(),
            year: Some(2001),
            poster_url: Some("/poster.jpg".to_string()),
        }
    }

    fn test_pending() -> PendingIdentification {
        PendingIdentification {
            file_key: "test-file-key".to_string(),
            original_path: "/movies/test.mkv".to_string(),
            original_filename: "test.mkv".to_string(),
            candidates: vec![
                test_candidate(12600, "Pokémon 4Ever"),
                test_candidate(12601, "Another Movie"),
            ],
        }
    }

    #[test]
    fn saves_and_retrieves_pending_identification() {
        let mut connection = open_in_memory().unwrap();
        let pending = test_pending();

        save_pending_identification(&mut connection, &pending).unwrap();

        let retrieved = get_pending_identification(&connection, "test-file-key").unwrap();

        assert_eq!(retrieved, Some(pending));
    }

    #[test]
    fn replaces_existing_candidates_when_resaving() {
        let mut connection = open_in_memory().unwrap();
        let mut pending = test_pending();

        save_pending_identification(&mut connection, &pending).unwrap();

        pending.candidates = vec![test_candidate(12600, "Pokémon 4Ever")];

        save_pending_identification(&mut connection, &pending).unwrap();

        let retrieved = get_pending_identification(&connection, "test-file-key")
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.candidates.len(), 1);
        assert_eq!(retrieved.candidates[0].tmdb_id, 12600);
    }

    #[test]
    fn resolves_pending_identification_and_removes_candidates() {
        let mut connection = open_in_memory().unwrap();
        let pending = test_pending();

        save_pending_identification(&mut connection, &pending).unwrap();

        let identification = IdentificationRecord {
            tmdb_id: 12600,
            media_type: TmdbMediaType::Movie,
            season: None,
            episode: None,
            episode_end: None,
            is_manual: true,
        };

        resolve_pending_identification(&mut connection, "test-file-key", &identification).unwrap();

        assert!(
            get_pending_identification(&connection, "test-file-key")
                .unwrap()
                .is_none()
        );

        let candidate_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM candidates WHERE file_key = ?1",
                ["test-file-key"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(candidate_count, 0);

        let stored_id: i64 = connection
            .query_row(
                "SELECT tmdb_id FROM identifications WHERE file_key = ?1",
                ["test-file-key"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stored_id, 12600);
    }

    #[test]
    fn lists_pending_identifications() {
        let mut connection = open_in_memory().unwrap();
        let pending = test_pending();

        save_pending_identification(&mut connection, &pending).unwrap();

        let listed = list_pending_identifications(&connection).unwrap();

        assert_eq!(listed, vec![pending]);
    }

    #[test]
    fn retrieves_existing_identification() {
        let mut connection = open_in_memory().unwrap();

        let identification = IdentificationRecord {
            tmdb_id: 157336,
            media_type: TmdbMediaType::Movie,
            season: None,
            episode: None,
            episode_end: None,
            is_manual: false,
        };

        resolve_pending_identification(&mut connection, "matrix-file", &identification).unwrap();

        let retrieved = get_identification(&connection, "matrix-file").unwrap();

        assert_eq!(retrieved, Some(identification));
    }

    #[test]
    fn returns_none_for_unknown_file_key() {
        let connection = open_in_memory().unwrap();

        let result = get_pending_identification(&connection, "missing").unwrap();

        assert_eq!(result, None);
    }
}
