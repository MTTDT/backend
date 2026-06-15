mod auth;
mod errors;
mod handlers;
mod models;
mod regression_model;
mod state;
mod stocks_register;


use axum::{
    http::{HeaderValue, StatusCode, Method},
    routing::{get, post, delete, patch},
    response::IntoResponse,
    Json, Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tower_http::cors::{Any, CorsLayer};

use state::AppState;

async fn cors_preflight() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    // ── Logging ────────────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=debug,tower_http=info".into()),
        )
        .init();

    // ── Environment ────────────────────────────────────────────────────────────
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
    .unwrap_or_else(|_| "sqlite::memory:".to_string());

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set in environment (use a long random string)");

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid port number");

    // ── Database ───────────────────────────────────────────────────────────────
    let db = SqlitePoolOptions::new()
    .max_connections(10)
    .connect("sqlite::memory:")
    .await
    .expect("Failed to connect to SQLite");

    // Run migrations automatically at startup
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database ready");

    // ── App State ──────────────────────────────────────────────────────────────
    let state = AppState::new(db, jwt_secret);

    // ── CORS ───────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse::<HeaderValue>().unwrap(),
            "https://stockmarketplacesim.vercel.app"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://stock-marketplace-sim-yq9dzs.rfcloud.cc"  
            .parse::<HeaderValue>()
            .unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    // ── Router ─────────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/auth/me", get(handlers::auth::me))
        .route("/api/admin/users", get(handlers::admin::get_users))
        .route("/api/admin/users/{id}", delete(handlers::admin::delete_user))
        .route("/api/admin/users/{id}/role", patch(handlers::admin::change_role))
        .route("/stocks/names", get(handlers::stocks::get_names))
        .route("/stocks", post(handlers::stocks::get_stocks))
        .route("/stocks/add", post(handlers::stocks::add_stock))
        .route("/stocks/{id}", delete(handlers::stocks::delete_stock))
        .route("/predict", post(handlers::stocks::predict))
        .fallback(|method: Method| async move {
            if method == Method::OPTIONS {
                StatusCode::OK.into_response()
            } else {
                (StatusCode::NOT_FOUND, "Not found").into_response()
            }
        })
        .with_state(state)
        .layer(cors);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app).await.unwrap();
}

async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "message": "Stocks API" }))
}

async fn health() -> &'static str {
    "ok"
}