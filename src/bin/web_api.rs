use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
};

use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;

use moviebox_tui::{
    providers::{ReleaseProvider, models::ProviderKind},
    service::MovieBoxService,
};

type AppState = Arc<MovieBoxService>;

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamQuery {
    se: Option<usize>,
    ep: Option<usize>,
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

            // Use MovieBox-Tui's existing local DASH proxy so the
            // browser does not access the signed CDN manifest directly.
            let proxy_url = moviebox_tui::proxy::spawn_sidecar(&direct_url, &headers, None).ok()?;

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

    let app = Router::new()
        .route("/health", get(health))
        .route("/home", get(home))
        .route("/search", get(search))
        .route("/search/suggest", get(suggestions))
        .route("/detail/{id}", get(details))
        .route("/api/stream/{subject_id}", get(stream))
        .layer(CorsLayer::permissive())
        .with_state(service);

    let address = "0.0.0.0:8000";

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
