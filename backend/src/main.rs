use auto_forge::provider::{ClaudeProvider, AIProviderState};
use auto_forge::rbac::db::RbacDb;
use auto_forge::rbac::middleware::{auth_middleware, RbacMiddlewareState};

use axum::http::Method;
use axum::Router;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;

#[derive(Clone)]
struct AppState {
    ai_provider: AIProviderState,
}

impl axum::extract::FromRef<AppState> for AIProviderState {
    fn from_ref(state: &AppState) -> Self {
        state.ai_provider.clone()
    }
}

// ---------------------------------------------------------------------------
// JWT secret helpers
// ---------------------------------------------------------------------------

/// Load JWT secret from {data_dir}/autoforge/jwt_secret.txt.
/// If the file doesn't exist, generate a random 64-char hex string and save it.
fn load_or_create_jwt_secret() -> String {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("jwt_secret.txt");

    if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_else(|_| generate_random_secret())
    } else {
        let secret = generate_random_secret();
        let _ = std::fs::write(&path, &secret);
        tracing::info!("Generated new JWT secret at {}", path.display());
        secret
    }
}

fn generate_random_secret() -> String {
    use std::fmt::Write;
    let bytes: [u8; 32] = rand::random();
    let mut hex = String::with_capacity(64);
    for b in &bytes {
        write!(&mut hex, "{:02x}", b).unwrap();
    }
    hex
}

// ---------------------------------------------------------------------------
// Seed default admin user
// ---------------------------------------------------------------------------

fn seed_default_admin(db: &RbacDb) {
    // Ensure default roles exist
    if db.get_role_by_name("admin").ok().flatten().is_none() {
        let _ = db.create_role("admin", "Administrator with full access");
    }
    if db.get_role_by_name("editor").ok().flatten().is_none() {
        let _ = db.create_role("editor", "Standard editor role");
    }

    // Create default admin user if not exists
    if db.get_user_by_username("admin").ok().flatten().is_none() {
        if let Ok(hash) = auto_forge::rbac::auth::hash_password("admin") {
            if let Ok(user) = db.create_user("admin", &hash) {
                if let Some(Some(admin_role)) = db.get_role_by_name("admin").ok() {
                    let _ = db.assign_role(user.id, admin_role.id);
                }
                tracing::info!("Seeded default admin user (admin/admin)");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Default RBAC database path
// ---------------------------------------------------------------------------

fn default_rbac_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
        .join("rbac.db")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("auto_forge=debug,tower_http=debug")
        .init();

    // ── RBAC initialization ──────────────────────────────────────────────
    let jwt_secret = load_or_create_jwt_secret();
    let rbac_db_path = default_rbac_db_path();
    if let Some(parent) = rbac_db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let rbac_db = RbacDb::open(&rbac_db_path)
        .expect("Failed to open RBAC database");
    seed_default_admin(&rbac_db);

    let rbac_mw_state = RbacMiddlewareState {
        db: rbac_db,
        jwt_secret: jwt_secret.clone(),
    };

    // ── App state ────────────────────────────────────────────────────────
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

    // Warm up LLM connection pool in the background so the first real request
    // does not pay the TCP+TLS handshake cost.
    let ai_provider_warmup = app_state.ai_provider.clone();
    tokio::spawn(async move {
        ai_provider_warmup.warm_up().await;
    });

    // ── Public routes (auth middleware with whitelist for login/register)
    let public_routes = Router::new()
        .merge(auto_forge::rbac::api::rbac_api_routes())
        .with_state(rbac_mw_state.clone())
        .layer(axum::middleware::from_fn_with_state(
            rbac_mw_state.clone(),
            auth_middleware,
        ));

    // ── Protected routes (auth required) ─────────────────────────────────
    let protected_routes = Router::new()
        .merge(auto_forge::forge::routes())
        .with_state(app_state)
        .layer(axum::middleware::from_fn_with_state(
            rbac_mw_state.clone(),
            auth_middleware,
        ));

    let avatars_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
        .join("avatars");

    let mut app = public_routes.merge(protected_routes);
    if forge_dist_dir.exists() {
        app = app.nest_service("/forge", tower_http::services::ServeDir::new(&forge_dist_dir));
        tracing::info!("AutoForge UI served at /forge ({})", forge_dist_dir.display());
    }
    app = app.nest_service("/avatars", tower_http::services::ServeDir::new(&avatars_dir));

    // MCP Streamable HTTP endpoint
    let mcp_server = auto_forge::mcp::AutoForgeMcpServer::new(ai_provider_clone.clone());
    let mcp_service = StreamableHttpService::new(
        move || Ok(mcp_server.clone()),
        std::sync::Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    app = app.route_service("/mcp", mcp_service);
    tracing::info!("MCP server mounted at /mcp");

    let app = app.layer(cors);

    // Start specs file watcher (replaces polling with native OS file-system events)
    auto_forge::forge::start_specs_watcher();

    // Restore last opened project from config
    auto_forge::forge::restore_last_project();

    // Resume any relay runs that were in Running state when we shut down
    auto_forge::relay::api::resume_running_runs(ai_provider_clone);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3031));
    tracing::info!("AutoForge server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
