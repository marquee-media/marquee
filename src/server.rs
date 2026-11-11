use crate::{database, matching::TmdbMediaType};
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone)]
struct AppState {
    database_path: PathBuf,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    application: &'static str,
}

#[derive(Serialize)]
struct LibraryItem {
    file_key: String,
    tmdb_id: i64,
    media_type: &'static str,
    season: Option<u32>,
    episode: Option<u32>,
    episode_end: Option<u32>,
    is_manual: bool,
    locations: Vec<MediaLocation>,
}

#[derive(Serialize)]
struct MediaLocation {
    path: String,
    root_path: Option<String>,
    is_primary: bool,
}

#[derive(Serialize)]
struct PendingItem {
    file_key: String,
    original_path: String,
    original_filename: String,
    candidates: Vec<PendingCandidate>,
}

#[derive(Serialize)]
struct PendingCandidate {
    tmdb_id: i64,
    media_type: &'static str,
    title: String,
    year: Option<u16>,
    poster_url: Option<String>,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        application: "marquee",
    })
}

async fn list_library(
    State(state): State<AppState>,
) -> Result<Json<Vec<LibraryItem>>, (StatusCode, String)> {
    let indexed_media = tokio::task::spawn_blocking(move || {
        let connection = database::open(state.database_path)?;
        database::list_identified_files(&connection)
    })
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database task failed: {error}"),
        )
    })?
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database query failed: {error}"),
        )
    })?;

    let items = indexed_media
        .into_iter()
        .map(|media| LibraryItem {
            file_key: media.file_key,
            tmdb_id: media.identification.tmdb_id,
            media_type: match media.identification.media_type {
                TmdbMediaType::Movie => "movie",
                TmdbMediaType::Tv => "tv",
            },
            season: media.identification.season,
            episode: media.identification.episode,
            episode_end: media.identification.episode_end,
            is_manual: media.identification.is_manual,
            locations: media
                .locations
                .into_iter()
                .map(|location| MediaLocation {
                    path: location.path,
                    root_path: location.root_path,
                    is_primary: location.is_primary,
                })
                .collect(),
        })
        .collect();

    Ok(Json(items))
}

async fn list_pending(
    State(state): State<AppState>,
) -> Result<Json<Vec<PendingItem>>, (StatusCode, String)> {
    let pending = tokio::task::spawn_blocking(move || {
        let connection = database::open(state.database_path)?;
        database::list_pending_identifications(&connection)
    })
    .await
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database task failed: {error}"),
        )
    })?
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database query failed: {error}"),
        )
    })?;

    let items = pending
        .into_iter()
        .map(|item| PendingItem {
            file_key: item.file_key,
            original_path: item.original_path,
            original_filename: item.original_filename,
            candidates: item
                .candidates
                .into_iter()
                .map(|candidate| PendingCandidate {
                    tmdb_id: candidate.tmdb_id,
                    media_type: match candidate.media_type {
                        TmdbMediaType::Movie => "movie",
                        TmdbMediaType::Tv => "tv",
                    },
                    title: candidate.title,
                    year: candidate.year,
                    poster_url: candidate.poster_url,
                })
                .collect(),
        })
        .collect();

    Ok(Json(items))
}

pub async fn run(database_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState { database_path };

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/library", get(list_library))
        .route("/api/pending", get(list_pending))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    println!("Marquee server listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
