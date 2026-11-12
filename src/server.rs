use crate::{database, matching::TmdbMediaType};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use std::{io::SeekFrom, path::PathBuf};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

#[derive(Debug, PartialEq, Eq)]
struct ByteRange {
    start: u64,
    end: u64,
}

fn parse_byte_range(value: &str, file_size: u64) -> Option<ByteRange> {
    if file_size == 0 {
        return None;
    }

    let range = value.strip_prefix("bytes=")?;

    // We support one byte range per request.
    if range.contains(',') {
        return None;
    }

    let (start, end) = range.split_once('-')?;

    if start.is_empty() {
        let suffix_length = end.parse::<u64>().ok()?;

        if suffix_length == 0 {
            return None;
        }

        return Some(ByteRange {
            start: file_size.saturating_sub(suffix_length),
            end: file_size - 1,
        });
    }

    let start = start.parse::<u64>().ok()?;

    if start >= file_size {
        return None;
    }

    let end = if end.is_empty() {
        file_size - 1
    } else {
        end.parse::<u64>().ok()?.min(file_size - 1)
    };

    if end < start {
        return None;
    }

    Some(ByteRange { start, end })
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/style.css"),
    )
}

async fn script() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../web/app.js"),
    )
}

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
    title: Option<String>,
    year: Option<u16>,
    overview: Option<String>,
    poster_path: Option<String>,
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
    poster_path: Option<String>,
}

async fn stream_media(
    State(state): State<AppState>,
    AxumPath(file_key): AxumPath<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let location = tokio::task::spawn_blocking(move || {
        let connection =
            database::open(state.database_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        database::get_identified_file_location(&connection, &file_key)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;

    stream_file(PathBuf::from(location.path), headers).await
}

async fn stream_file(path: PathBuf, headers: HeaderMap) -> Result<Response, StatusCode> {
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let metadata = file
        .metadata()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !metadata.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }

    let file_size = metadata.len();

    let range = headers.get(header::RANGE).map(|value| {
        value
            .to_str()
            .ok()
            .and_then(|value| parse_byte_range(value, file_size))
    });

    if headers.contains_key(header::RANGE) && range.is_none() {
        let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();

        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes */{file_size}"))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        );

        response
            .headers_mut()
            .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));

        return Ok(response);
    }

    let (status, start, length) = match range {
        Some(Some(range)) => (
            StatusCode::PARTIAL_CONTENT,
            range.start,
            range.end - range.start + 1,
        ),
        Some(None) => unreachable!(),
        None => (StatusCode::OK, 0, file_size),
    };

    let mut file = file;

    file.seek(SeekFrom::Start(start))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::new(Body::from_stream(stream));

    *response.status_mut() = status;

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let content_type = match extension.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "ogv" => "video/ogg",
        "wmv" => "video/x-ms-wmv",
        "ts" => "video/mp2t",
        _ => "application/octet-stream",
    };

    let response_headers = response.headers_mut();

    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response_headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    if status == StatusCode::PARTIAL_CONTENT {
        response_headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{}/{file_size}", start + length - 1))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        );
    }

    Ok(response)
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
            title: media
                .metadata
                .as_ref()
                .map(|metadata| metadata.title.clone()),
            year: media.metadata.as_ref().and_then(|metadata| metadata.year),
            overview: media
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.overview.clone()),
            poster_path: media
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.poster_path.clone()),
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
                    poster_path: candidate.poster_path,
                })
                .collect(),
        })
        .collect();

    Ok(Json(items))
}

pub async fn run(database_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let state = AppState { database_path };

    let app = Router::new()
        .route("/", get(index))
        .route("/style.css", get(stylesheet))
        .route("/app.js", get(script))
        .route("/api/health", get(health_check))
        .route("/api/library", get(list_library))
        .route("/api/pending", get(list_pending))
        .route("/stream/{file_key}", get(stream_media))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    println!("Marquee server listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
