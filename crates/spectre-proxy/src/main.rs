use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, error};
use spectre_secrets::SecretManager;
use secrecy::ExposeSecret;


#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    role: String,
    exp: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize Logging
    tracing_subscriber::fmt::init();
    info!("Starting Spectre Proxy (Hardened + Middleware)...");

    // 2. Load Critical Secrets
    // We strictly require this to fail if missing
    let jwt_secret = SecretManager::get("JWT_SECRET")
        .expect("CRITICAL: JWT_SECRET must be set")
        .expose_secret()
        .clone();
    
    // Store secret in a way accessible to middleware (State or Extension)
    // For simplicity here, we'll clone it into the closure (or use simple static/state)
    let state = AppState { jwt_secret };

    // 3. Setup Router with Middleware
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/ingest", post(ingest_event))
        .route("/api/v1/neutron/*path",  post(proxy_to_neutron).get(proxy_to_neutron)) // Catch-all for neutron
        // Apply Global Middleware
        .layer(TraceLayer::new_for_http())
        // Apply Auth Middleware to API routes
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state);

    // 4. Bind
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Spectre Proxy listening on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    jwt_secret: String,
}

async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Extract Header
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = &auth_header[7..];

    // 2. Verify JWT
    // In production, cache decoding keys
    let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
    let validation = Validation::default();

    match decode::<Claims>(token, &key, &validation) {
        Ok(token_data) => {
            info!("Authenticated Subject: {}", token_data.claims.sub);
            // Inject claims into request extensions for handlers to use
            req.extensions_mut().insert(token_data.claims);
            Ok(next.run(req).await)
        }
        Err(e) => {
            warn!("JWT Validation Failed: {}", e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

async fn health_check() -> &'static str {
    "OK"
}

async fn ingest_event() -> &'static str {
    "Event Ingested Securely"
}

// Proxy Handler
use axum::extract::Path;
use axum::response::IntoResponse;

async fn proxy_to_neutron(
    Path(path): Path<String>,
    req: Request<Body>,
) -> Result<impl IntoResponse, StatusCode> {
    let client = reqwest::Client::new();
    let neutron_url = format!("http://localhost:8000/api/v1/{}", path);

    info!("Proxying to Neutron: {}", neutron_url);

    // 1. Extract Body (Bytes)
    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024) // 10MB limit
        .await
        .map_err(|e| {
            error!("Failed to read body: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // 2. Forward Request
    let res = client.post(&neutron_url)
        .body(body_bytes)
        .header("Content-Type", "application/json") // Should forward original headers ideally
        .send()
        .await
        .map_err(|e| {
            error!("Neutron upstream error: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    // 3. Extract Response
    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let res_bytes = res.bytes().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((status, res_bytes))
}
