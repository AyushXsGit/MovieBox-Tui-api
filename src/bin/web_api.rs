use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use axum::{
    Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{any, get, post},
};

use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;

use moviebox_tui::{
    providers::{ReleaseProvider, models::ProviderKind},
    service::MovieBoxService,
};

#[derive(Clone)]
struct DownloadState {
    jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
}

#[derive(Clone)]
struct DownloadJob {
    status: String,
    progress: f64,
    downloaded_seconds: f64,
    total_seconds: f64,
    speed: String,
    size: String,
    output: Option<String>,
    filename: String,
    error: Option<String>,
    cancel_requested: Arc<AtomicBool>,
}

type AppState = Arc<MovieBoxService>;

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamQuery {
    se: Option<usize>,
    ep: Option<usize>,
    quality: Option<String>,
}

fn api_error(error: impl ToString) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "error": error.to_string()
        })),
    )
}

// ======================================================
// HEALTH
// ======================================================

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "Movie Web API"
    }))
}

// ======================================================
// SEARCH
// ======================================================

async fn search(
    State(service): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let query = params.q.unwrap_or_default();

    if query.trim().is_empty() {
        return Ok(Json(json!({
            "items": []
        })));
    }

    let items = service
        .search_typed(ProviderKind::MovieBox, &query, 1)
        .await
        .map_err(api_error)?;

    Ok(Json(json!({
        "query": query,
        "items": items
    })))
}

// ======================================================
// SEARCH SUGGESTIONS
// ======================================================

async fn suggestions(
    State(service): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let query = params.q.unwrap_or_default();

    if query.trim().is_empty() {
        return Ok(Json(json!({
            "suggestions": []
        })));
    }

    let items = service
        .search_typed(ProviderKind::MovieBox, &query, 1)
        .await
        .map_err(api_error)?;

    let suggestions: Vec<Value> = items
        .into_iter()
        .take(8)
        .map(|item| {
            json!({
                "title": item.title,
                "slug": item.id.value,
                "subject_id": item.id.value,
                "subjectId": item.id.value
            })
        })
        .collect();

    Ok(Json(json!({
        "suggestions": suggestions
    })))
}

// ======================================================
// MOVIE DETAILS
// ======================================================

async fn details(
    State(service): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let details = service
        .details_typed(ProviderKind::MovieBox, &id)
        .await
        .map_err(api_error)?;

    let seasons: Vec<Value> = details
        .seasons
        .iter()
        .map(|season| {
            json!({
                "se": season.number,
                "season": season.number,
                "maxEp": season.episodes.len()
            })
        })
        .collect();

    let dubs: Vec<Value> = details
        .dubs
        .iter()
        .map(|dub| {
            json!({
                "subject_id": dub.subject_id,
                "lanName": dub.language,
                "lanCode": dub.label
            })
        })
        .collect();

    let subject_type = if details.is_series() { 2 } else { 1 };

    let subject = json!({
        "subjectId": details.id.value,
        "title": details.title,
        "subjectType": subject_type,
        "releaseDate": details.year,
        "description": details.description,
        "tagline": details.tagline,
        "imdbRatingValue": details.imdb_rating,
        "director": details.director,
        "stars": details.stars,
        "prints": details.prints,
        "audios": details.audios,

        "cover": {
            "url": details.poster_url
        },

        "genre": details.genres
    });

    Ok(Json(json!({
        "data": {
            "subject": subject,

            "resource": {
                "seasons": seasons
            },

            "dubs": dubs
        }
    })))
}

// ======================================================
// HOME
// ======================================================

async fn home(State(service): State<AppState>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (items, _metrics) = service.homepage("2", 1).await.map_err(api_error)?;

    Ok(Json(json!({
        "sections": [
            {
                "section": "Featured",
                "items": items
            }
        ]
    })))
}

// ======================================================
// STREAM
// ======================================================

async fn stream(
    State(service): State<AppState>,
    Path(subject_id): Path<String>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let season = params.se.unwrap_or(0);
    let episode = params.ep.unwrap_or(0);

    let releases = service
        .client
        .episode_streams(&subject_id, season, episode)
        .await
        .map_err(api_error)?;

    let sources: Vec<Value> = releases
        .into_iter()
        .filter_map(|release| {
            let direct_url = release.direct_url().map(|value| value.to_string())?;

            let headers = release
                .mirrors
                .first()
                .map(|mirror| mirror.headers.clone())
                .unwrap_or_default();

            // Register a public proxy on this API service so browsers
            // can reach the stream through Render instead of localhost.
            let proxy_url =
                moviebox_tui::proxy::register_public_proxy(&direct_url, &headers, None).ok()?;

            Some(json!({
                "url": proxy_url,
                "proxy_url": proxy_url,
                "direct_url": direct_url,
                "quality": release.quality,
                "codec": release.codec,
                "language": release.language,
                "season": release.season,
                "episode": release.episode,
                "source": release.source_label()
            }))
        })
        .collect();

    if sources.is_empty() {
        return Err(api_error("No playable sources found"));
    }

    Ok(Json(json!({
        "sources": sources
    })))
}

// ======================================================
// DOWNLOAD
// ======================================================

async fn download(
    State(service): State<AppState>,
    Path(subject_id): Path<String>,
    Query(params): Query<StreamQuery>,
) -> Result<(StatusCode, [(axum::http::HeaderName, String); 2], Vec<u8>), (StatusCode, Json<Value>)> {
    let season = params.se.unwrap_or(0);
    let episode = params.ep.unwrap_or(0);

    let releases = service
        .client
        .episode_streams(&subject_id, season, episode)
        .await
        .map_err(api_error)?;

    let requested_quality = params.quality.as_deref();

    let release = if let Some(quality) = requested_quality {
        releases
            .iter()
            .find(|release| {
                release.direct_url().is_some()
                    && release
                        .quality
                        .as_deref()
                        .map(|value| value.eq_ignore_ascii_case(quality))
                        .unwrap_or(false)
            })
            .or_else(|| releases.iter().find(|release| release.direct_url().is_some()))
            .cloned()
    } else {
        releases
            .iter()
            .find(|release| release.direct_url().is_some())
            .cloned()
    }
    .ok_or_else(|| api_error("No downloadable source found"))?;

    let direct_url = release
        .direct_url()
        .map(|value| value.to_string())
        .ok_or_else(|| api_error("Selected source has no direct URL"))?;

    let headers = release
        .mirrors
        .first()
        .map(|mirror| mirror.headers.clone())
        .unwrap_or_default();

    let download_url =
        moviebox_tui::proxy::register_public_proxy(&direct_url, &headers, None)
            .map_err(api_error)?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let filename = format!(
        "movie-{}-{}-{}-{}.mp4",
        subject_id,
        season,
        episode,
        stamp
    );

    let temp_dir = std::env::var("TMPDIR")
        .or_else(|_| std::env::var("TMP"))
        .or_else(|_| std::env::var("TEMP"))
        .or_else(|_| {
            std::env::var("PREFIX")
                .map(|prefix| format!("{}/tmp", prefix.trim_end_matches('/')))
        })
        .unwrap_or_else(|_| "/tmp".to_string());

    let output = format!("{}/{}", temp_dir.trim_end_matches('/'), filename);

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&download_url)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&output)
        .status()
        .await
        .map_err(|error| api_error(format!("Failed to start FFmpeg: {}", error)))?;

    if !status.success() {
        return Err(api_error(format!(
            "FFmpeg failed with status: {}",
            status
        )));
    }

    let data = tokio::fs::read(&output)
        .await
        .map_err(|error| api_error(format!("Failed to read downloaded file: {}", error)))?;

    let _ = tokio::fs::remove_file(&output).await;

    let content_type = "video/mp4".to_string();
    let disposition = format!("attachment; filename=\"{}\"", filename);

    println!(
        "Download complete: {} (quality={:?}, codec={:?})",
        filename, release.quality, release.codec
    );

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
        ],
        data,
    ))
}

// ======================================================
// START BACKGROUND DOWNLOAD
// ======================================================

async fn start_download(
    State(service): State<AppState>,
    Extension(download_state): Extension<DownloadState>,
    Path(subject_id): Path<String>,
    Query(params): Query<StreamQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let season = params.se.unwrap_or(0);
    let episode = params.ep.unwrap_or(0);

    let releases = service
        .client
        .episode_streams(&subject_id, season, episode)
        .await
        .map_err(api_error)?;

    let requested_quality = params.quality.as_deref();

    let release = if let Some(quality) = requested_quality {
        releases
            .iter()
            .find(|release| {
                release.direct_url().is_some()
                    && release
                        .quality
                        .as_deref()
                        .map(|value| value.eq_ignore_ascii_case(quality))
                        .unwrap_or(false)
            })
            .or_else(|| releases.iter().find(|release| release.direct_url().is_some()))
            .cloned()
    } else {
        releases
            .iter()
            .find(|release| release.direct_url().is_some())
            .cloned()
    }
    .ok_or_else(|| api_error("No downloadable source found"))?;

    let direct_url = release
        .direct_url()
        .map(|value| value.to_string())
        .ok_or_else(|| api_error("Selected source has no direct URL"))?;

    let headers = release
        .mirrors
        .first()
        .map(|mirror| mirror.headers.clone())
        .unwrap_or_default();

    let download_url =
        moviebox_tui::proxy::register_public_proxy(&direct_url, &headers, None)
            .map_err(api_error)?;

    let total_seconds = match Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(&download_url)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<f64>()
                .unwrap_or(0.0)
        }
        _ => 0.0,
    };


    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let job_id = format!("{}-{}-{}", subject_id, stamp, episode);

    let filename = format!(
        "movie-{}-{}-{}-{}.mp4",
        subject_id,
        season,
        episode,
        stamp
    );

    let temp_dir = std::env::var("TMPDIR")
        .or_else(|_| std::env::var("TMP"))
        .or_else(|_| std::env::var("TEMP"))
        .or_else(|_| {
            std::env::var("PREFIX")
                .map(|prefix| format!("{}/tmp", prefix.trim_end_matches('/')))
        })
        .unwrap_or_else(|_| "/tmp".to_string());

    let output = format!("{}/{}", temp_dir.trim_end_matches('/'), filename);

    let cancel_requested = Arc::new(AtomicBool::new(false));

    let job = DownloadJob {
        status: "starting".to_string(),
        progress: 0.0,
        downloaded_seconds: 0.0,
        total_seconds,
        speed: String::new(),
        size: String::new(),
        output: None,
        filename: filename.clone(),
        error: None,
        cancel_requested: cancel_requested.clone(),
    };

    {
        let mut jobs = download_state
            .jobs
            .lock()
            .map_err(|_| api_error("Download manager unavailable"))?;

        jobs.insert(job_id.clone(), job);
    }

    let jobs = download_state.jobs.clone();
    let job_id_for_task = job_id.clone();
    let output_for_task = output.clone();
    let cancel_for_task = cancel_requested.clone();

    tokio::spawn(async move {
        let result = async {
            let mut child = Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(&download_url)
                .arg("-c")
                .arg("copy")
                .arg("-progress")
                .arg("pipe:1")
                .arg("-nostats")
                .arg(&output_for_task)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|error| format!("Failed to start FFmpeg: {}", error))?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| "Failed to read FFmpeg progress".to_string())?;

            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| "Failed to read FFmpeg error output".to_string())?;

            let stderr_task = tokio::spawn(async move {
                let mut stderr = stderr;
                let mut error_output = String::new();
                stderr.read_to_string(&mut error_output).await
                    .map_err(|error| format!("Failed to read FFmpeg error output: {}", error))?;
                Ok::<String, String>(error_output)
            });

            let mut lines = BufReader::new(stdout).lines();

            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|error| format!("Failed to read FFmpeg progress: {}", error))?
            {
                if cancel_for_task.load(Ordering::Relaxed) {
                    let _ = child.kill().await;

                    if let Ok(mut jobs) = jobs.lock() {
                        if let Some(job) = jobs.get_mut(&job_id_for_task) {
                            job.status = "cancelled".to_string();
                            job.error = None;
                        }
                    }

                    return Ok::<(), String>(());
                }

                if let Some(value) = line.strip_prefix("out_time_us=") {
                    if let Ok(us) = value.parse::<f64>() {
                        if let Ok(mut jobs) = jobs.lock() {
                            if let Some(job) = jobs.get_mut(&job_id_for_task) {
                                job.downloaded_seconds = us / 1_000_000.0;

                                if job.total_seconds > 0.0 {
                                    job.progress =
                                        (job.downloaded_seconds / job.total_seconds * 100.0)
                                            .clamp(0.0, 100.0);
                                }
                            }
                        }
                    }
                }

                if let Some(value) = line.strip_prefix("speed=") {
                    if let Ok(mut jobs) = jobs.lock() {
                        if let Some(job) = jobs.get_mut(&job_id_for_task) {
                            job.speed = value.to_string();
                            job.status = "downloading".to_string();
                        }
                    }
                }

                if let Some(value) = line.strip_prefix("total_size=") {
                    if let Ok(bytes) = value.parse::<u64>() {
                        if let Ok(mut jobs) = jobs.lock() {
                            if let Some(job) = jobs.get_mut(&job_id_for_task) {
                                job.size = if bytes >= 1024 * 1024 {
                                    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
                                } else {
                                    format!("{:.1} KB", bytes as f64 / 1024.0)
                                };
                            }
                        }
                    }
                }

                if line == "progress=end" {
                    break;
                }
            }

            let status = child
                .wait()
                .await
                .map_err(|error| format!("Failed waiting for FFmpeg: {}", error))?;

            let error_output = stderr_task
                .await
                .map_err(|error| format!("FFmpeg error reader failed: {}", error))??;

            if !status.success() {
                let details = error_output.trim();
                if details.is_empty() {
                    return Err(format!("FFmpeg failed with status: {}", status));
                }
                return Err(format!("FFmpeg failed with status: {}: {}", status, details));
            }

            Ok::<(), String>(())
        }
        .await;

        if let Ok(mut jobs) = jobs.lock() {
            if let Some(job) = jobs.get_mut(&job_id_for_task) {
                match result {
                    Ok(()) if cancel_for_task.load(Ordering::Relaxed) => {
                        job.status = "cancelled".to_string();
                        job.error = None;
                        job.output = None;
                    }
                    Ok(()) => {
                        job.status = "completed".to_string();
                        job.progress = 100.0;
                        job.output = Some(output_for_task.clone());
                    }
                    Err(error) => {
                        job.status = "error".to_string();
                        job.error = Some(error);
                    }
                }
            }
        }
    });

    Ok(Json(json!({
        "job_id": job_id,
        "status": "starting",
        "filename": filename,
    })))
}

// ======================================================
// DOWNLOAD STATUS
// ======================================================

async fn download_status(
    Extension(download_state): Extension<DownloadState>,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let jobs = download_state
        .jobs
        .lock()
        .map_err(|_| api_error("Download manager unavailable"))?;

    let job = jobs
        .get(&job_id)
        .ok_or_else(|| api_error("Download job not found"))?;

    Ok(Json(json!({
        "job_id": job_id,
        "status": job.status,
        "progress": job.progress,
        "downloaded_seconds": job.downloaded_seconds,
        "total_seconds": job.total_seconds,
        "speed": job.speed,
        "size": job.size,
        "filename": job.filename,
        "ready": job.output.is_some(),
        "error": job.error,
    })))
}

// ======================================================
// DOWNLOAD FILE
// ======================================================
async fn cancel_download(
    Extension(download_state): Extension<DownloadState>,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let jobs = download_state
        .jobs
        .lock()
        .map_err(|_| api_error("Download manager unavailable"))?;

    let job = jobs
        .get(&job_id)
        .ok_or_else(|| api_error("Download job not found"))?;

    if job.status == "completed" {
        return Err(api_error("Download is already completed"));
    }

    if job.status == "cancelled" {
        return Ok(Json(json!({
            "job_id": job_id,
            "status": "cancelled"
        })));
    }

    job.cancel_requested.store(true, Ordering::Relaxed);

    Ok(Json(json!({
        "job_id": job_id,
        "status": "cancelling"
    })))
}

async fn download_file(
    Extension(download_state): Extension<DownloadState>,
    Path(job_id): Path<String>,
) -> Result<(StatusCode, [(axum::http::HeaderName, String); 2], axum::body::Body), (StatusCode, Json<Value>)> {
    let output = {
        let jobs = download_state
            .jobs
            .lock()
            .map_err(|_| api_error("Download manager unavailable"))?;

        let job = jobs
            .get(&job_id)
            .ok_or_else(|| api_error("Download job not found"))?;

        if job.status != "completed" {
            return Err(api_error("Download is not completed yet"));
        }

        job.output
            .clone()
            .ok_or_else(|| api_error("Download file is not available"))?
    };

    let file = tokio::fs::File::open(&output)
        .await
        .map_err(|error| api_error(format!("Could not open download file: {}", error)))?;

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let filename = {
        let jobs = download_state
            .jobs
            .lock()
            .map_err(|_| api_error("Download manager unavailable"))?;

        jobs.get(&job_id)
            .map(|job| job.filename.clone())
            .ok_or_else(|| api_error("Download job not found"))?
    };

    Ok((
        StatusCode::OK,
        [
            (
                axum::http::header::CONTENT_TYPE,
                "video/mp4".to_string(),
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\" ", filename).trim_end().to_string(),
            ),
        ],
        body,
    ))
}

// ======================================================
// MAIN
// ======================================================

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // MovieBox-Tui's DASH proxy sidecar.
    if let Some(pos) = args.iter().position(|arg| arg == "--proxy-for-vlc") {
        let target_url = args.get(pos + 1).cloned().unwrap_or_default();
        let headers_json = args
            .get(pos + 2)
            .cloned()
            .unwrap_or_else(|| "[]".to_string());
        let subtitle_url = args.get(pos + 3).cloned().filter(|value| !value.is_empty());

        let headers: Vec<(String, String)> =
            serde_json::from_str(&headers_json).unwrap_or_default();

        moviebox_tui::proxy::run_sidecar(target_url, headers, subtitle_url).await;

        return;
    }

    println!("Starting Movie Web API...");

    let service = Arc::new(MovieBoxService::new());
    let download_state = DownloadState {
        jobs: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/home", get(home))
        .route("/search", get(search))
        .route("/search/suggest", get(suggestions))
        .route("/detail/{id}", get(details))
        .route("/api/stream/{subject_id}", get(stream))
        .route("/api/download/{subject_id}", get(download))
        .route(
            "/api/download/start/{subject_id}",
            get(start_download),
        )
        .route("/api/download/status/{job_id}", get(download_status))
        .route("/api/download/cancel/{job_id}", post(cancel_download))
        .route("/api/download/file/{job_id}", get(download_file))
        .route(
            "/public-proxy/{*path}",
            any(moviebox_tui::proxy::public_proxy),
        )
        .layer(CorsLayer::permissive())
        .layer(axum::Extension(download_state))
        .with_state(service);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let address = format!("0.0.0.0:{}", port);

    println!();
    println!("Movie Web API running at:");
    println!("http://127.0.0.1:8000");
    println!();

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("Could not start API server");

    axum::serve(listener, app)
        .await
        .expect("API server stopped");
}
