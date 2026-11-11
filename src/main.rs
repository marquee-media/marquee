use marquee::{database, file_key, filename, identification, matching, scanner, tmdb};

use std::env;
use std::path::PathBuf;

fn main() {
    let mut args = env::args_os().skip(1);

    let Some(command) = args.next() else {
        print_usage();
        println!("  marquee migrate");
        return;
    };

    match command.to_string_lossy().as_ref() {
        "migrate" => {
            let mut connection = match open_default_database() {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("Could not open Marquee database: {error}");
                    std::process::exit(1);
                }
            };

            let version: i64 =
                match connection.query_row("PRAGMA user_version", [], |row| row.get(0)) {
                    Ok(version) => version,
                    Err(error) => {
                        eprintln!("Could not read database version: {error}");
                        std::process::exit(1);
                    }
                };

            if version > 2 {
                eprintln!(
                    "Database version {version} is newer than this Marquee version supports."
                );
                std::process::exit(1);
            }

            if version == 0 {
                match database::migrate_file_keys(&mut connection) {
                    Ok(count) => {
                        println!("Migration 1 complete: migrated {count} file records.");
                    }
                    Err(error) => {
                        eprintln!("Migration 1 failed: {error}");
                        std::process::exit(1);
                    }
                }
            }

            let version: i64 =
                match connection.query_row("PRAGMA user_version", [], |row| row.get(0)) {
                    Ok(version) => version,
                    Err(error) => {
                        eprintln!("Could not read database version: {error}");
                        std::process::exit(1);
                    }
                };

            if version == 1 {
                match database::migrate_root_paths(&mut connection) {
                    Ok(()) => println!("Migration 2 complete: added library root tracking."),
                    Err(error) => {
                        eprintln!("Migration 2 failed: {error}");
                        std::process::exit(1);
                    }
                }
            } else if version == 2 {
                println!("Database is already up to date (version 2).");
            }
        }

        "scan" => {
            let directories: Vec<PathBuf> = args.map(PathBuf::from).collect();

            if directories.is_empty() {
                eprintln!("Please provide at least one directory to scan.");
                print_usage();
                std::process::exit(1);
            }

            let mut total_files = 0;
            let mut total_errors = 0;

            for directory in directories {
                println!("\nScanning: {}", directory.display());

                let result = scanner::scan_directory(&directory);

                for file in &result.files {
                    let extension = file.extension.as_deref().unwrap_or("[no extension]");
                    println!("  {} ({})", file.path.display(), extension);
                }

                for error in &result.errors {
                    eprintln!("  ERROR: {}: {}", error.path.display(), error.message);
                }

                println!(
                    "\nFound {} files and {} errors.",
                    result.files.len(),
                    result.errors.len()
                );

                total_files += result.files.len();
                total_errors += result.errors.len();
            }

            println!("\n--- Scan summary ---");
            println!("Total files: {total_files}");
            println!("Total errors: {total_errors}");

            if total_errors > 0 {
                std::process::exit(1);
            }
        }

        "parse" => {
            let Some(path) = args.next() else {
                eprintln!("Please provide a filename to parse.");
                std::process::exit(1);
            };

            let candidate = filename::parse_filename(&PathBuf::from(path));

            println!("Original: {}", candidate.original_name);
            println!("Title: {}", candidate.title);

            println!(
                "Year: {}",
                candidate
                    .year
                    .map_or_else(|| "Unknown".to_string(), |year| year.to_string())
            );

            println!("Confident: {}", candidate.confident);

            match candidate.kind {
                filename::MediaKind::Movie => {
                    println!("Type: Movie candidate");
                }

                filename::MediaKind::Episode {
                    season,
                    episodes,
                    episode_title,
                } => {
                    println!("Type: Episode numbering candidate");
                    println!(
                        "Season: {}",
                        season.map_or_else(|| "Unknown".to_string(), |n| n.to_string())
                    );
                    println!(
                        "Episodes: {}",
                        if episodes.is_empty() {
                            "Unknown".to_string()
                        } else {
                            episodes
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    );
                    println!(
                        "Episode title: {}",
                        episode_title.unwrap_or_else(|| "Unknown".to_string())
                    );
                }

                filename::MediaKind::NumberedItem {
                    identifier,
                    episode_title,
                } => {
                    println!("Type: Unclassified numbered item");
                    println!("Original identifier: {identifier}");
                    println!(
                        "Episode title: {}",
                        episode_title.unwrap_or_else(|| "Unknown".to_string())
                    );
                }

                filename::MediaKind::Unknown => {
                    println!("Type: Unknown");
                }
            }
        }

        "preview" => {
            let directories: Vec<PathBuf> = args.map(PathBuf::from).collect();

            if directories.is_empty() {
                eprintln!("Please provide at least one directory to preview.");
                print_usage();
                std::process::exit(1);
            }

            let mut total_movies = 0;
            let mut total_episodes = 0;
            let mut total_numbered_items = 0;
            let mut total_unknown = 0;
            let mut total_errors = 0;

            for directory in directories {
                println!("\n========================================");
                println!("Previewing: {}", directory.display());
                println!("========================================");

                let result = identification::prepare_directory(&directory);

                let mut movies = 0;
                let mut episodes_count = 0;
                let mut numbered_items = 0;
                let mut unknown = 0;

                for candidate in &result.candidates {
                    println!("\n{}", candidate.path.display());
                    println!("  Title: {}", candidate.parsed.title);

                    if let Some(year) = candidate.parsed.year {
                        println!("  Year: {year}");
                    } else {
                        println!("  Year: Unknown");
                    }

                    match &candidate.parsed.kind {
                        filename::MediaKind::Movie => {
                            movies += 1;
                            println!("  Type: Movie candidate");
                        }

                        filename::MediaKind::Episode {
                            season,
                            episodes,
                            episode_title,
                        } => {
                            episodes_count += 1;
                            println!("  Type: Episode numbering candidate");

                            if let Some(season) = season {
                                println!("  Season: {season}");
                            }

                            if !episodes.is_empty() {
                                println!(
                                    "  Episodes: {}",
                                    episodes
                                        .iter()
                                        .map(u32::to_string)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                            }

                            if let Some(title) = episode_title {
                                println!("  Episode title: {title}");
                            }
                        }

                        filename::MediaKind::NumberedItem {
                            identifier,
                            episode_title,
                        } => {
                            numbered_items += 1;
                            println!("  Type: Unclassified numbered item");
                            println!("  Original identifier: {identifier}");

                            if let Some(title) = episode_title {
                                println!("  Episode title: {title}");
                            }
                        }

                        filename::MediaKind::Unknown => {
                            unknown += 1;
                            println!("  Type: Unknown");
                        }
                    }

                    println!(
                        "  Confidence: {}",
                        if candidate.parsed.confident {
                            "Confident"
                        } else {
                            "Uncertain"
                        }
                    );
                }

                for error in &result.errors {
                    eprintln!("\n  ERROR: {}: {}", error.path.display(), error.message);
                }

                println!("\n--- Directory summary ---");
                println!("Movies: {movies}");
                println!("TV episode candidates: {episodes_count}");
                println!("Unclassified numbered items: {numbered_items}");
                println!("Uncertain / unknown: {unknown}");
                println!("Scanning errors: {}", result.errors.len());

                total_movies += movies;
                total_episodes += episodes_count;
                total_numbered_items += numbered_items;
                total_unknown += unknown;
                total_errors += result.errors.len();
            }

            println!("\n========================================");
            println!("MARQUEE PREVIEW SUMMARY");
            println!("========================================");
            println!("Movies: {total_movies}");
            println!("TV episode candidates: {total_episodes}");
            println!("Unclassified numbered items: {total_numbered_items}");
            println!("Uncertain / unknown: {total_unknown}");
            println!("Scanning errors: {total_errors}");
            println!(
                "Total video files: {}",
                total_movies + total_episodes + total_numbered_items + total_unknown
            );
            println!("\nNo files were modified.");
        }

        "identify" => {
            let directories: Vec<PathBuf> = args.map(PathBuf::from).collect();

            if directories.is_empty() {
                eprintln!("Please provide at least one directory to identify.");
                print_usage();
                std::process::exit(1);
            }

            let client = match tmdb::TmdbClient::from_env() {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("Could not initialize TMDb client: {error}");
                    std::process::exit(1);
                }
            };

            let mut connection = match open_default_database() {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("Could not open Marquee database: {error}");
                    std::process::exit(1);
                }
            };

            let mut total_files = 0;
            let mut automatic_matches = 0;
            let mut already_identified = 0;
            let mut already_pending = 0;
            let mut manual_identification = 0;
            let mut search_failures = 0;
            let mut scanning_errors = 0;
            let mut database_errors = 0;

            println!("\nMarquee identification scan");
            println!("===========================");

            for directory in directories {
                println!("\nScanning: {}", directory.display());

                let result = identification::prepare_directory(&directory);

                let root_path = match directory.canonicalize() {
                    Ok(path) => path.to_string_lossy().into_owned(),
                    Err(error) => {
                        scanning_errors += 1;
                        eprintln!("  ! Could not resolve library root: {error}");
                        continue;
                    }
                };

                let database_errors_before = database_errors;
                let mut root_scan_complete = result.errors.is_empty();
                let mut scanned_paths = Vec::with_capacity(result.candidates.len());

                for candidate in &result.candidates {
                    total_files += 1;

                    let candidate_path = match candidate.path.canonicalize() {
                        Ok(path) => path,
                        Err(error) => {
                            root_scan_complete = false;
                            scanning_errors += 1;
                            eprintln!(
                                "  ! Could not resolve file path {}: {error}",
                                candidate.path.display()
                            );
                            continue;
                        }
                    };

                    let candidate_path_string = candidate_path.to_string_lossy().into_owned();

                    let file_key = match file_key::create(&candidate_path) {
                        Ok(key) => key,
                        Err(error) => {
                            root_scan_complete = false;
                            database_errors += 1;
                            eprintln!(
                                "  ! Could not create file key for {}: {error}",
                                candidate.path.display()
                            );
                            continue;
                        }
                    };

                    scanned_paths.push(candidate_path_string.clone());

                    match database::register_file_location(
                        &mut connection,
                        &file_key,
                        &candidate_path_string,
                        &root_path,
                    ) {
                        Ok(_) => {}
                        Err(error) => {
                            root_scan_complete = false;
                            database_errors += 1;
                            eprintln!(
                                "  ! Could not register location for {}: {error}",
                                candidate.path.display()
                            );
                            continue;
                        }
                    }

                    match database::get_identification(&connection, &file_key) {
                        Ok(Some(existing)) => {
                            already_identified += 1;
                            println!("  ↻ Already identified (TMDb ID: {})", existing.tmdb_id);
                            continue;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            database_errors += 1;
                            eprintln!(
                                "  ! Could not check database for {}: {error}",
                                candidate.path.display()
                            );
                            continue;
                        }
                    }

                    match database::get_pending_identification(&connection, &file_key) {
                        Ok(Some(_)) => {
                            let filename = candidate.parsed.original_name.as_str();

                            match database::update_pending_location(
                                &connection,
                                &file_key,
                                &candidate_path_string,
                                filename,
                            ) {
                                Ok(true) => {
                                    already_pending += 1;
                                    println!("  ↻ Already awaiting manual identification");
                                }
                                Ok(false) => {
                                    database_errors += 1;
                                    eprintln!(
                                        "  ! Could not update pending location for {}",
                                        candidate.path.display()
                                    );
                                }
                                Err(error) => {
                                    database_errors += 1;
                                    eprintln!(
                                        "  ! Could not update pending location for {}: {error}",
                                        candidate.path.display()
                                    );
                                }
                            }

                            continue;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            database_errors += 1;
                            eprintln!(
                                "  ! Could not check pending identification for {}: {error}",
                                candidate.path.display()
                            );
                            continue;
                        }
                    }

                    let outcome = identification::identify_candidate(candidate, &client);

                    match outcome.status {
                        identification::IdentificationStatus::Matched(
                            matching::MatchResult::Automatic(matched),
                        ) => {
                            let record = create_identification_record(candidate, &matched);

                            match database::resolve_pending_identification(
                                &mut connection,
                                &file_key,
                                &record,
                            ) {
                                Ok(()) => {
                                    automatic_matches += 1;

                                    let title = match matched.media_type {
                                        matching::TmdbMediaType::Movie => {
                                            matched.result.title.as_deref()
                                        }
                                        matching::TmdbMediaType::Tv => {
                                            matched.result.name.as_deref()
                                        }
                                    }
                                    .unwrap_or("Unknown title");

                                    let date = match matched.media_type {
                                        matching::TmdbMediaType::Movie => {
                                            matched.result.release_date.as_deref()
                                        }
                                        matching::TmdbMediaType::Tv => {
                                            matched.result.first_air_date.as_deref()
                                        }
                                    };

                                    if let Some(year) = date.and_then(|date| date.get(..4)) {
                                        println!("  ✓ {title} ({year})");
                                    } else {
                                        println!("  ✓ {title}");
                                    }
                                }
                                Err(error) => {
                                    database_errors += 1;
                                    eprintln!(
                                        "  ! Could not save identification for {}: {error}",
                                        candidate.path.display()
                                    );
                                }
                            }
                        }

                        identification::IdentificationStatus::Matched(
                            matching::MatchResult::NeedsSelection(results),
                        ) => {
                            let pending =
                                create_pending_identification(candidate, &file_key, &results);

                            match database::save_pending_identification(&mut connection, &pending) {
                                Ok(()) => manual_identification += 1,
                                Err(error) => {
                                    database_errors += 1;
                                    eprintln!(
                                        "  ! Could not save review candidates for {}: {error}",
                                        candidate.path.display()
                                    );
                                }
                            }
                        }

                        identification::IdentificationStatus::Matched(
                            matching::MatchResult::NoMatch,
                        )
                        | identification::IdentificationStatus::ManualReview => {
                            let pending = create_pending_identification(candidate, &file_key, &[]);

                            match database::save_pending_identification(&mut connection, &pending) {
                                Ok(()) => manual_identification += 1,
                                Err(error) => {
                                    database_errors += 1;
                                    eprintln!(
                                        "  ! Could not save pending file {}: {error}",
                                        candidate.path.display()
                                    );
                                }
                            }
                        }

                        identification::IdentificationStatus::SearchFailed(error) => {
                            search_failures += 1;
                            eprintln!(
                                "  ! TMDb search failed for {}: {error}",
                                candidate.path.display()
                            );
                        }
                    }
                }

                scanning_errors += result.errors.len();

                for error in &result.errors {
                    eprintln!(
                        "  ! Scanning error: {}: {}",
                        error.path.display(),
                        error.message
                    );
                }

                if root_scan_complete && database_errors == database_errors_before {
                    match database::reconcile_file_locations(
                        &mut connection,
                        &root_path,
                        &scanned_paths,
                    ) {
                        Ok(0) => {}
                        Ok(removed) => {
                            println!("  Reconciled {removed} stale file location(s).");
                        }
                        Err(error) => {
                            database_errors += 1;
                            eprintln!("  ! Could not reconcile library root: {error}");
                        }
                    }
                } else {
                    println!("  Skipping location reconciliation because the scan was incomplete.");
                }
            }

            println!("\n===========================");
            println!("IDENTIFICATION SUMMARY");
            println!("===========================");
            println!("Video files scanned: {total_files}");
            println!("Automatically identified: {automatic_matches}");
            println!("Already identified: {already_identified}");
            println!("Already awaiting manual identification: {already_pending}");
            println!("New files needing manual identification: {manual_identification}");
            println!("TMDb search failures: {search_failures}");
            println!("Scanning errors: {scanning_errors}");
            println!("Database errors: {database_errors}");
            println!("\nMedia files were not modified.");
        }

        "resolve" => {
            let Some(path) = args.next() else {
                eprintln!("Please provide the file path.");
                std::process::exit(1);
            };

            let Some(tmdb_id) = args.next() else {
                eprintln!("Please provide a TMDb ID.");
                std::process::exit(1);
            };

            let Some(media_type) = args.next() else {
                eprintln!("Please specify 'movie' or 'tv'.");
                std::process::exit(1);
            };

            let tmdb_id = match tmdb_id.to_string_lossy().parse::<i64>() {
                Ok(id) => id,
                Err(_) => {
                    eprintln!("Invalid TMDb ID.");
                    std::process::exit(1);
                }
            };

            let media_type = match media_type.to_string_lossy().as_ref() {
                "movie" => matching::TmdbMediaType::Movie,
                "tv" => matching::TmdbMediaType::Tv,
                _ => {
                    eprintln!("Media type must be 'movie' or 'tv'.");
                    std::process::exit(1);
                }
            };

            let path = PathBuf::from(path);

            let file_key = match file_key::create(&path) {
                Ok(key) => key,
                Err(error) => {
                    eprintln!("Could not access file: {error}");
                    std::process::exit(1);
                }
            };

            let mut connection = match open_default_database() {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("Could not open Marquee database: {error}");
                    std::process::exit(1);
                }
            };

            let pending = match database::get_pending_identification(&connection, &file_key) {
                Ok(Some(pending)) => pending,
                Ok(None) => {
                    eprintln!("This file is not awaiting manual identification.");
                    std::process::exit(1);
                }
                Err(error) => {
                    eprintln!("Could not retrieve pending identification: {error}");
                    std::process::exit(1);
                }
            };

            let selected = pending.candidates.iter().find(|candidate| {
                candidate.tmdb_id == tmdb_id && candidate.media_type == media_type
            });

            let Some(selected) = selected else {
                eprintln!(
                    "TMDb ID {tmdb_id} ({media_type:?}) is not a saved candidate for this file."
                );
                std::process::exit(1);
            };

            let parsed = filename::parse_filename(&path);

            let (season, episode, episode_end) = match parsed.kind {
                filename::MediaKind::Episode {
                    season, episodes, ..
                } => {
                    let first = episodes.first().copied();
                    let last = episodes.last().copied();
                    let end = last.filter(|last| Some(*last) != first);

                    (season, first, end)
                }
                _ => (None, None, None),
            };

            let record = database::IdentificationRecord {
                tmdb_id: selected.tmdb_id,
                media_type: selected.media_type,
                season,
                episode,
                episode_end,
                is_manual: true,
            };

            match database::resolve_pending_identification(&mut connection, &file_key, &record) {
                Ok(()) => {
                    println!("Manual identification saved:");
                    println!("  File: {}", pending.original_filename);
                    println!("  Title: {}", selected.title);

                    if let Some(year) = selected.year {
                        println!("  Year: {year}");
                    }

                    println!("  TMDb ID: {}", selected.tmdb_id);
                    println!("  Type: {:?}", selected.media_type);
                    println!("  Pending candidates removed.");
                }
                Err(error) => {
                    eprintln!("Could not save identification: {error}");
                    std::process::exit(1);
                }
            }
        }

        "tmdb-test" => {
            let Some(media_type) = args.next() else {
                eprintln!("Please specify 'movie' or 'tv'.");
                std::process::exit(1);
            };

            let query = args
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");

            if query.trim().is_empty() {
                eprintln!("Please provide a search query.");
                std::process::exit(1);
            }

            let client = match tmdb::TmdbClient::from_env() {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("Could not initialize TMDb client: {error}");
                    std::process::exit(1);
                }
            };

            let results = match media_type.to_string_lossy().as_ref() {
                "movie" => client.search_movies(&query, None),
                "tv" => client.search_tv(&query, None),
                _ => {
                    eprintln!("Media type must be 'movie' or 'tv'.");
                    std::process::exit(1);
                }
            };

            match results {
                Ok(results) => {
                    println!("\nTMDb search results for: {query}");
                    println!("Found {} results.\n", results.len());

                    for result in results {
                        let title = result
                            .title
                            .or(result.name)
                            .unwrap_or_else(|| "Unknown title".to_string());
                        let year = result
                            .release_date
                            .or(result.first_air_date)
                            .and_then(|date| date.get(..4).map(str::to_string))
                            .unwrap_or_else(|| "Unknown year".to_string());

                        println!("{title} ({year})");
                        println!("  TMDb ID: {}", result.id);

                        if let Some(overview) = result.overview
                            && !overview.is_empty()
                        {
                            println!("  {overview}");
                        }

                        println!();
                    }
                }
                Err(error) => {
                    eprintln!("TMDb search failed: {error}");
                    std::process::exit(1);
                }
            }
        }

        _ => {
            eprintln!("Unknown command.");
            print_usage();
            std::process::exit(1);
        }
    }
}

fn open_default_database() -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    let data_directory = match env::var_os("XDG_DATA_HOME") {
        Some(directory) if !directory.is_empty() => PathBuf::from(directory),
        _ => PathBuf::from(env::var_os("HOME").ok_or("Could not determine home directory")?)
            .join(".local/share"),
    };

    let application_directory = data_directory.join("marquee");
    std::fs::create_dir_all(&application_directory)?;

    Ok(database::open(application_directory.join("marquee.db"))?)
}

fn create_pending_identification(
    candidate: &identification::IdentificationCandidate,
    file_key: &str,
    results: &[matching::TmdbCandidate],
) -> database::PendingIdentification {
    let candidates = results
        .iter()
        .map(|result| {
            let (title, date) = match result.media_type {
                matching::TmdbMediaType::Movie => (
                    result.result.title.as_deref().unwrap_or("Unknown title"),
                    result.result.release_date.as_deref(),
                ),
                matching::TmdbMediaType::Tv => (
                    result.result.name.as_deref().unwrap_or("Unknown title"),
                    result.result.first_air_date.as_deref(),
                ),
            };

            database::StoredCandidate {
                tmdb_id: result.result.id as i64,
                media_type: result.media_type,
                title: title.to_string(),
                year: date
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse::<u16>().ok()),
                poster_url: result
                    .result
                    .poster_path
                    .as_deref()
                    .map(|path| format!("https://image.tmdb.org/t/p/w500{path}")),
            }
        })
        .collect();

    database::PendingIdentification {
        file_key: file_key.to_string(),
        original_path: candidate.path.to_string_lossy().into_owned(),
        original_filename: candidate.parsed.original_name.clone(),
        candidates,
    }
}

fn create_identification_record(
    candidate: &identification::IdentificationCandidate,
    matched: &matching::TmdbCandidate,
) -> database::IdentificationRecord {
    let (season, episode, episode_end) = match &candidate.parsed.kind {
        filename::MediaKind::Episode {
            season, episodes, ..
        } => {
            let first = episodes.first().copied();
            let last = episodes.last().copied();

            let end = last.filter(|last| Some(*last) != first);

            (*season, first, end)
        }
        _ => (None, None, None),
    };

    database::IdentificationRecord {
        tmdb_id: matched.result.id as i64,
        media_type: matched.media_type,
        season,
        episode,
        episode_end,
        is_manual: false,
    }
}

fn print_usage() {
    println!("Marquee - Local-first media server");
    println!();
    println!("Usage:");
    println!("  marquee scan <directory> [directory ...]");
    println!("  marquee parse <filename>");
    println!("  marquee preview <directory> [directory ...]");
    println!("  marquee identify <directory> [directory ...]");
    println!("  marquee resolve <file> <tmdb_id> <movie|tv>");
    println!("  marquee tmdb-test <movie|tv> <search query>");
}
