use auto_forge::provider::{ClaudeProvider, AIProviderState};
use auto_forge::rbac::db::RbacDb;
use auto_forge::rbac::middleware::{auth_middleware, RbacMiddlewareState};

use axum::body::Body;
use axum::extract::Request;
use axum::http::Method;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
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

/// Reverse-proxy handler for Vite dev server.
/// Forwards all `/forge/*` requests to `http://localhost:5174`.
async fn vite_proxy(req: Request) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let target_url = format!("http://localhost:5174{}", path_and_query);
    let method = req.method().clone();
    let headers = req.headers().clone();

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Vite proxy failed to read request body: {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let mut reqwest_req = client.request(method, &target_url);
    for (k, v) in headers {
        if let Some(k) = k {
            if k.as_str().to_lowercase() != "host" {
                reqwest_req = reqwest_req.header(k, v);
            }
        }
    }

    let response = match reqwest_req.body(body_bytes).send().await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!("Vite proxy request failed: {}", e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let status = response.status();
    let mut builder = Response::builder().status(status);
    for (k, v) in response.headers() {
        builder = builder.header(k, v);
    }

    let body_bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };

    match builder.body(Body::from(body_bytes)) {
        Ok(resp) => resp,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

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

    // ── Vite dev server proxy or static files ────────────────────────────
    let mut app = public_routes.merge(protected_routes);
    let vite_running = tokio::net::TcpStream::connect("127.0.0.1:5174").await.is_ok();
    if vite_running {
        app = app.route("/forge", axum::routing::any(vite_proxy))
                 .route("/forge/{*path}", axum::routing::any(vite_proxy));
        tracing::info!("AutoForge UI proxied to Vite dev server at http://localhost:5174/forge/");
    } else if forge_dist_dir.exists() {
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
