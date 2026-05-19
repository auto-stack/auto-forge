use auto_forge::provider::{ClaudeProvider, AIProviderState};

use axum::http::Method;
use axum::Router;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    ai_provider: AIProviderState,
}

impl axum::extract::FromRef<AppState> for AIProviderState {
    fn from_ref(state: &AppState) -> Self {
        state.ai_provider.clone()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("auto_forge=debug,tower_http=debug")
        .init();

    // AutoForge UI static files
    let forge_dist_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("frontend")
        .join("dist");
    let forge_dist_dir = forge_dist_dir.canonicalize().unwrap_or(forge_dist_dir);

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    let app_state = AppState {
        ai_provider: std::sync::Arc::new(ClaudeProvider::new()),
    };
    let ai_provider_clone = app_state.ai_provider.clone();

    let api_routes = Router::new()
        .merge(auto_forge::forge::routes())
        .with_state(app_state);

    let avatars_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
        .join("avatars");

    let mut app = api_routes;
    if forge_dist_dir.exists() {
        app = app.nest_service("/forge", tower_http::services::ServeDir::new(&forge_dist_dir));
        tracing::info!("AutoForge UI served at /forge ({})", forge_dist_dir.display());
    }
    app = app.nest_service("/avatars", tower_http::services::ServeDir::new(&avatars_dir));

    let app = app.layer(cors);

    // Start periodic specs reload task (picks up disk edits and derives statuses)
    auto_forge::forge::start_periodic_reload();

    // Restore last opened project from config
    auto_forge::forge::restore_last_project();

    // Resume any relay runs that were in Running state when we shut down
    auto_forge::relay::api::resume_running_runs(ai_provider_clone);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3031));
    tracing::info!("AutoForge server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
