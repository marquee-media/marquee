use std::path::Path;

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};

use crate::matching::TmdbMediaType;

use crate::file_key;
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

/// A known location of a media file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLocation {
    pub file_key: String,
    pub path: String,
    pub root_path: Option<String>,
    pub is_primary: bool,
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

    CREATE TABLE IF NOT EXISTS file_locations (
        file_key TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        root_path TEXT,
        is_primary INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (file_key, path)
    );

    CREATE UNIQUE INDEX IF NOT EXISTS idx_file_locations_primary
    ON file_locations(file_key)
    WHERE is_primary = 1;
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

/// Updates the stored location and filename of a pending file.
///
/// Existing TMDb candidates are preserved.
pub fn update_pending_location(
    connection: &Connection,
    file_key: &str,
    path: &str,
    filename: &str,
) -> rusqlite::Result<bool> {
    let updated = connection.execute(
        "
        UPDATE pending_identifications
        SET original_path = ?1,
        original_filename = ?2
        WHERE file_key = ?3
        ",
        params![path, filename, file_key],
    )?;

    Ok(updated == 1)
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

/// Registers a location for a file fingerprint.
///
/// The first registered location becomes primary.
/// Additional locations are recorded as duplicates.
/// Existing locations without a root are assigned one when encountered.
pub fn register_file_location(
    connection: &mut Connection,
    file_key: &str,
    path: &str,
    root_path: &str,
) -> rusqlite::Result<bool> {
    let transaction = connection.transaction()?;

    let existing: Option<(String, bool)> = transaction
        .query_row(
            "
        SELECT file_key, is_primary
        FROM file_locations
        WHERE path = ?1
        ",
            [path],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
        )
        .optional()?;

    if let Some((existing_key, is_primary)) = existing {
        if existing_key == file_key {
            transaction.execute(
                "
                UPDATE file_locations
                SET root_path = ?1
                WHERE path = ?2
                ",
                params![root_path, path],
            )?;

            transaction.commit()?;
            return Ok(is_primary);
        }

        transaction.execute("DELETE FROM file_locations WHERE path = ?1", [path])?;

        if is_primary {
            transaction.execute(
                "
                UPDATE file_locations
                SET is_primary = 1
                WHERE rowid = (
                    SELECT rowid
                    FROM file_locations
                    WHERE file_key = ?1
                    ORDER BY path
                    LIMIT 1
            )
            ",
                [&existing_key],
            )?;
        }
    }

    let has_primary: bool = transaction.query_row(
        "
        SELECT EXISTS(
            SELECT 1
            FROM file_locations
            WHERE file_key = ?1 AND is_primary = 1
    )
    ",
        [file_key],
        |row| row.get(0),
    )?;

    let is_primary = !has_primary;

    transaction.execute(
        "
        INSERT INTO file_locations (file_key, path, root_path, is_primary)
    VALUES (?1, ?2, ?3, ?4)
    ",
        params![file_key, path, root_path, i64::from(is_primary)],
    )?;

    transaction.commit()?;

    Ok(is_primary)
}

/// Removes locations under a scanned root that were not found in the scan.
///
/// Call this only after a complete, successful scan of the root.
/// If a removed location was primary, another remaining location is promoted.
/// This function never modifies actual media files.
pub fn reconcile_file_locations(
    connection: &mut Connection,
    root_path: &str,
    scanned_paths: &[String],
) -> rusqlite::Result<usize> {
    let scanned_paths: HashSet<&str> = scanned_paths.iter().map(String::as_str).collect();

    let transaction = connection.transaction()?;

    let stale_locations = {
        let mut statement = transaction.prepare(
            "
            SELECT file_key, path, is_primary
            FROM file_locations
            WHERE root_path = ?1
            ",
        )?;

        let rows = statement.query_map([root_path], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })?;

        let mut stale = Vec::new();

        for row in rows {
            let (file_key, path, is_primary) = row?;

            if !scanned_paths.contains(path.as_str()) {
                stale.push((file_key, path, is_primary));
            }
        }

        stale
    };

    let mut affected_keys = HashSet::new();

    for (file_key, path, _) in &stale_locations {
        transaction.execute(
            "
            DELETE FROM file_locations
            WHERE file_key = ?1 AND path = ?2 AND root_path = ?3
            ",
            params![file_key, path, root_path],
        )?;

        affected_keys.insert(file_key.clone());
    }

    for file_key in affected_keys {
        let has_primary: bool = transaction.query_row(
            "
            SELECT EXISTS(
                SELECT 1
                FROM file_locations
                WHERE file_key = ?1 AND is_primary = 1
        )
        ",
            [&file_key],
            |row| row.get(0),
        )?;

        if !has_primary {
            transaction.execute(
                "
                UPDATE file_locations
                SET is_primary = 1
                WHERE rowid = (
                    SELECT rowid
                    FROM file_locations
                    WHERE file_key = ?1
                    ORDER BY path
                    LIMIT 1
            )
            ",
                [&file_key],
            )?;
        }
    }

    let removed = stale_locations.len();

    transaction.commit()?;

    Ok(removed)
}

/// Retrieves every known location for a file fingerprint.
pub fn get_file_locations(
    connection: &Connection,
    file_key: &str,
) -> rusqlite::Result<Vec<FileLocation>> {
    let mut statement = connection.prepare(
        "
        SELECT file_key, path, root_path, is_primary
        FROM file_locations
        WHERE file_key = ?1
        ORDER BY is_primary DESC, path
        ",
    )?;

    let rows = statement.query_map([file_key], |row| {
        Ok(FileLocation {
            file_key: row.get(0)?,
            path: row.get(1)?,
            root_path: row.get(2)?,
            is_primary: row.get::<_, i64>(3)? != 0,
        })
    })?;

    rows.collect()
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

/// Migrates legacy path-based file keys to content fingerprints.
///
/// The migration is atomic: if a file is missing, a fingerprint collides
/// with another record, or any database operation fails, nothing is changed.
pub fn migrate_file_keys(connection: &mut Connection) -> Result<usize, Box<dyn std::error::Error>> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version >= 1 {
        return Err("File-key migration has already been applied.".into());
    }

    let mut identification_rows = Vec::new();

    {
        let mut statement =
            connection.prepare("SELECT file_key FROM identifications ORDER BY file_key")?;

        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

        for row in rows {
            identification_rows.push(row?);
        }
    }

    let mut pending_rows = Vec::new();

    {
        let mut statement = connection.prepare(
            "SELECT file_key, original_path FROM pending_identifications ORDER BY file_key",
        )?;

        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            pending_rows.push(row?);
        }
    }

    // Calculate every new key before changing the database.
    let mut migrations = Vec::new();
    let mut new_keys = std::collections::HashSet::new();

    for old_key in &identification_rows {
        let path = Path::new(old_key);
        let new_key = file_key::create(path)?;

        if !new_keys.insert(new_key.clone()) {
            return Err(
                format!("Fingerprint collision while migrating: {}", path.display()).into(),
            );
        }

        migrations.push((old_key.clone(), new_key, old_key.clone()));
    }

    for (old_key, path) in &pending_rows {
        let new_key = file_key::create(Path::new(path))?;

        if !new_keys.insert(new_key.clone()) {
            return Err(format!("Fingerprint collision while migrating: {path}").into());
        }

        migrations.push((old_key.clone(), new_key, path.clone()));
    }

    let transaction = connection.transaction()?;

    transaction.execute_batch("PRAGMA defer_foreign_keys = ON;")?;

    for (old_key, new_key, path) in &migrations {
        transaction.execute(
            "UPDATE candidates SET file_key = ?1 WHERE file_key = ?2",
            rusqlite::params![new_key, old_key],
        )?;

        transaction.execute(
            "UPDATE identifications SET file_key = ?1 WHERE file_key = ?2",
            rusqlite::params![new_key, old_key],
        )?;

        transaction.execute(
            "UPDATE pending_identifications SET file_key = ?1 WHERE file_key = ?2",
            rusqlite::params![new_key, old_key],
        )?;

        transaction.execute(
            "
            INSERT INTO file_locations (file_key, path, is_primary)
        VALUES (?1, ?2, 1)
        ",
            rusqlite::params![new_key, path],
        )?;
    }

    transaction.pragma_update(None, "user_version", 1)?;

    transaction.commit()?;

    Ok(migrations.len())
}

/// Adds root ownership to file locations.
pub fn migrate_root_paths(connection: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version >= 2 {
        return Err("Root-path migration has already been applied.".into());
    }

    if version != 1 {
        return Err("File-key migration must be applied before root-path migration.".into());
    }

    let transaction = connection.transaction()?;

    let has_root_path = {
        let mut statement = transaction.prepare("PRAGMA table_info(file_locations)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;

        let mut found = false;

        for column in columns {
            if column? == "root_path" {
                found = true;
                break;
            }
        }

        found
    };

    if !has_root_path {
        transaction.execute_batch("ALTER TABLE file_locations ADD COLUMN root_path TEXT;")?;
    }

    transaction.execute_batch("PRAGMA user_version = 2;")?;

    transaction.commit()?;

    Ok(())
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

    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn create_test_file() -> (std::path::PathBuf, String) {
        let id = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let path = std::env::temp_dir().join(format!(
            "marquee-migration-test-{}-{id}.mkv",
            std::process::id()
        ));

        fs::write(&path, b"test video content").unwrap();

        let key = file_key::create(&path).unwrap();

        (path, key)
    }

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
    fn updating_pending_location_preserves_candidates() {
        let mut connection = open_in_memory().unwrap();
        let pending = test_pending();

        save_pending_identification(&mut connection, &pending).unwrap();

        let updated = update_pending_location(
            &connection,
            "test-file-key",
            "/new/location/The Thing.mkv",
            "The Thing.mkv",
        )
        .unwrap();

        assert!(updated);

        let retrieved = get_pending_identification(&connection, "test-file-key")
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.original_path, "/new/location/The Thing.mkv");
        assert_eq!(retrieved.original_filename, "The Thing.mkv");
        assert_eq!(retrieved.candidates, pending.candidates);
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
    fn reconciliation_removes_stale_locations_from_root() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/old.mkv",
            "/movies",
        )
        .unwrap();

        let scanned_paths = vec!["/movies/new.mkv".to_string()];

        let removed = reconcile_file_locations(&mut connection, "/movies", &scanned_paths).unwrap();

        assert_eq!(removed, 1);

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();
        assert!(locations.is_empty());
    }

    #[test]
    fn reconciliation_preserves_locations_from_other_roots() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/backup/test.mkv",
            "/backup",
        )
        .unwrap();

        let scanned_paths = Vec::new();

        let removed = reconcile_file_locations(&mut connection, "/movies", &scanned_paths).unwrap();

        assert_eq!(removed, 1);

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].path, "/backup/test.mkv");
        assert!(locations[0].is_primary);
    }

    #[test]
    fn reconciliation_promotes_remaining_duplicate_when_primary_disappears() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/backup/test.mkv",
            "/backup",
        )
        .unwrap();

        let scanned_paths = Vec::new();

        let removed = reconcile_file_locations(&mut connection, "/movies", &scanned_paths).unwrap();

        assert_eq!(removed, 1);

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].path, "/backup/test.mkv");
        assert!(locations[0].is_primary);
    }

    #[test]
    fn first_file_location_becomes_primary() {
        let mut connection = open_in_memory().unwrap();

        let is_primary = register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        assert!(is_primary);

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();

        assert_eq!(locations.len(), 1);
        assert!(locations[0].is_primary);
        assert_eq!(locations[0].root_path.as_deref(), Some("/movies"));
    }

    #[test]
    fn additional_file_locations_become_duplicates() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        let is_primary = register_file_location(
            &mut connection,
            "fingerprint-1",
            "/backup/test.mkv",
            "/backup",
        )
        .unwrap();

        assert!(!is_primary);

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();

        assert_eq!(locations.len(), 2);
        assert_eq!(
            locations
                .iter()
                .filter(|location| location.is_primary)
                .count(),
            1
        );
    }

    #[test]
    fn registering_same_location_twice_is_idempotent() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-1",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        let locations = get_file_locations(&connection, "fingerprint-1").unwrap();

        assert_eq!(locations.len(), 1);
        assert!(locations[0].is_primary);
    }

    #[test]
    fn reusing_a_path_for_different_content_replaces_old_location() {
        let mut connection = open_in_memory().unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-old",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        register_file_location(
            &mut connection,
            "fingerprint-new",
            "/movies/test.mkv",
            "/movies",
        )
        .unwrap();

        let old_locations = get_file_locations(&connection, "fingerprint-old").unwrap();
        let new_locations = get_file_locations(&connection, "fingerprint-new").unwrap();

        assert!(old_locations.is_empty());
        assert_eq!(new_locations.len(), 1);
        assert!(new_locations[0].is_primary);
    }

    #[test]
    fn migrates_existing_identification_to_fingerprint() {
        let mut connection = open_in_memory().unwrap();
        let (path, expected_key) = create_test_file();

        let old_path = path.to_string_lossy().into_owned();

        let identification = IdentificationRecord {
            tmdb_id: 12600,
            media_type: TmdbMediaType::Movie,
            season: None,
            episode: None,
            episode_end: None,
            is_manual: false,
        };

        resolve_pending_identification(&mut connection, &old_path, &identification).unwrap();

        let migrated = migrate_file_keys(&mut connection).unwrap();

        assert_eq!(migrated, 1);

        let retrieved = get_identification(&connection, &expected_key)
            .unwrap()
            .unwrap();

        assert_eq!(retrieved, identification);

        let locations = get_file_locations(&connection, &expected_key).unwrap();

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].path, old_path);
        assert!(locations[0].is_primary);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn migrates_pending_identification_and_candidates() {
        let mut connection = open_in_memory().unwrap();
        let (path, expected_key) = create_test_file();

        let old_path = path.to_string_lossy().into_owned();

        let pending = PendingIdentification {
            file_key: old_path.clone(),
            original_path: old_path.clone(),
            original_filename: path.file_name().unwrap().to_string_lossy().into_owned(),
            candidates: vec![StoredCandidate {
                tmdb_id: 1091,
                media_type: TmdbMediaType::Movie,
                title: "The Thing".to_string(),
                year: Some(1982),
                poster_url: None,
            }],
        };

        save_pending_identification(&mut connection, &pending).unwrap();

        let migrated = migrate_file_keys(&mut connection).unwrap();

        assert_eq!(migrated, 1);

        let pending = get_pending_identification(&connection, &expected_key)
            .unwrap()
            .unwrap();

        assert_eq!(pending.file_key, expected_key);
        assert_eq!(pending.original_path, old_path);
        assert_eq!(pending.candidates.len(), 1);
        assert_eq!(pending.candidates[0].tmdb_id, 1091);
        assert_eq!(pending.candidates[0].title, "The Thing");

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn migration_rolls_back_when_file_is_missing() {
        let mut connection = open_in_memory().unwrap();

        let old_path = std::env::temp_dir()
            .join("marquee-missing-migration-test.mkv")
            .to_string_lossy()
            .into_owned();

        let identification = IdentificationRecord {
            tmdb_id: 12600,
            media_type: TmdbMediaType::Movie,
            season: None,
            episode: None,
            episode_end: None,
            is_manual: false,
        };

        resolve_pending_identification(&mut connection, &old_path, &identification).unwrap();

        let result = migrate_file_keys(&mut connection);

        assert!(result.is_err());

        let still_exists = get_identification(&connection, &old_path)
            .unwrap()
            .is_some();

        assert!(still_exists);

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 0);
    }

    #[test]
    fn refuses_to_run_migration_twice() {
        let mut connection = open_in_memory().unwrap();
        let (path, expected_key) = create_test_file();
        let old_path = path.to_string_lossy().into_owned();

        let identification = IdentificationRecord {
            tmdb_id: 12600,
            media_type: TmdbMediaType::Movie,
            season: None,
            episode: None,
            episode_end: None,
            is_manual: false,
        };

        resolve_pending_identification(&mut connection, &old_path, &identification).unwrap();

        assert_eq!(migrate_file_keys(&mut connection).unwrap(), 1);

        let second_attempt = migrate_file_keys(&mut connection);
        assert!(second_attempt.is_err());

        assert!(
            get_identification(&connection, &expected_key)
                .unwrap()
                .is_some()
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn returns_none_for_unknown_file_key() {
        let connection = open_in_memory().unwrap();

        let result = get_pending_identification(&connection, "missing").unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn migrates_version_one_database_to_version_two() {
        let mut connection = open_in_memory().unwrap();

        // Simulate a version 1 database without root_path.
        connection
            .execute_batch(
                "
            ALTER TABLE file_locations DROP COLUMN root_path;
            PRAGMA user_version = 1;
            ",
            )
            .unwrap();

        connection
            .execute(
                "
            INSERT INTO file_locations (file_key, path, is_primary)
        VALUES ('test-key', '/movies/test.mkv', 1)
        ",
                [],
            )
            .unwrap();

        migrate_root_paths(&mut connection).unwrap();

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 2);

        let location = connection
            .query_row(
                "
            SELECT file_key, path, root_path, is_primary
            FROM file_locations
            WHERE file_key = 'test-key'
        ",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(
            location,
            (
                "test-key".to_string(),
                "/movies/test.mkv".to_string(),
                None,
                true,
            )
        );
    }

    #[test]
    fn refuses_to_run_root_migration_before_version_one() {
        let mut connection = open_in_memory().unwrap();

        let result = migrate_root_paths(&mut connection);

        assert!(result.is_err());

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 0);
    }

    #[test]
    fn refuses_to_run_root_migration_twice() {
        let mut connection = open_in_memory().unwrap();

        connection
            .execute_batch("PRAGMA user_version = 1;")
            .unwrap();

        migrate_root_paths(&mut connection).unwrap();

        let second_attempt = migrate_root_paths(&mut connection);

        assert!(second_attempt.is_err());

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 2);
    }
}
