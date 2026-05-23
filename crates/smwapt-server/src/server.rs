use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use smwapt_core::package::Package;
use smwapt_core::registry::{load_repository, write_repository};
use smwapt_core::smwc::{sync_packages, SyncOptions};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub repo_dir: PathBuf,
    pub packages: Arc<RwLock<Vec<Package>>>,
}

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub repo_dir: PathBuf,
    pub bind: SocketAddr,
}

pub async fn run_server(options: ServerOptions) -> Result<()> {
    let packages = load_repository(&options.repo_dir)
        .map(|index| index.packages)
        .unwrap_or_default();
    let state = AppState {
        repo_dir: options.repo_dir.clone(),
        packages: Arc::new(RwLock::new(packages)),
    };
    let app = Router::new()
        .route("/api/v1/packages", get(list_packages))
        .route("/api/v1/search", get(search_packages))
        .route("/api/v1/packages/:name", get(package_details))
        .route("/api/v1/admin/sync", post(admin_sync))
        .nest_service("/", ServeDir::new(options.repo_dir))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(options.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list_packages(State(state): State<AppState>) -> Json<Vec<Package>> {
    Json(state.packages.read().expect("package lock").clone())
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

async fn search_packages(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<Package>> {
    let needle = query.q.unwrap_or_default().to_ascii_lowercase();
    let packages = state
        .packages
        .read()
        .expect("package lock")
        .iter()
        .filter(|pkg| {
            needle.is_empty()
                || pkg.name.contains(&needle)
                || pkg.title.to_ascii_lowercase().contains(&needle)
                || pkg.aliases.iter().any(|alias| alias.contains(&needle))
                || pkg.tags.iter().any(|tag| tag.to_ascii_lowercase().contains(&needle))
        })
        .cloned()
        .collect();
    Json(packages)
}

async fn package_details(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Package>, StatusCode> {
    state
        .packages
        .read()
        .expect("package lock")
        .iter()
        .find(|pkg| pkg.name == name || pkg.aliases.iter().any(|alias| alias == &name))
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[derive(Debug, Deserialize)]
struct SyncQuery {
    max_pages: Option<u64>,
    sections: Option<String>,
}

async fn admin_sync(
    State(state): State<AppState>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<Package>>, (StatusCode, String)> {
    let repo_dir = state.repo_dir.clone();
    let sections = query.sections.map(|sections| {
        sections
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });
    let options = SyncOptions {
        sections: sections.unwrap_or_else(|| SyncOptions::default().sections),
        max_pages: query.max_pages.or(Some(1)),
        ..SyncOptions::default()
    };
    let packages = tokio::task::spawn_blocking(move || {
        let packages = sync_packages(&options)?;
        write_repository(&repo_dir, packages.clone())?;
        anyhow::Ok(packages)
    })
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")))?;
    *state.packages.write().expect("package lock") = packages.clone();
    Ok(Json(packages))
}
