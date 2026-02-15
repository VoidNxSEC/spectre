use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use jsonwebtoken::{decode, DecodingKey, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use spectre_secrets::SecretManager;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};


// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    role: String,
    exp: usize,
}

/// Role hierarchy: admin > service > readonly
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Readonly = 0,
    Service = 1,
    Admin = 2,
}

impl Role {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Role::Admin),
            "service" => Some(Role::Service),
            "readonly" => Some(Role::Readonly),
            _ => None,
        }
    }
}

// ── Structured Error Response ──────────────────────────────────────────────

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
    message: String,
    status: u16,
}

struct ApiError {
    status: StatusCode,
    error: String,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            error: error.into(),
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "Unauthorized", message)
    }

    fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "Forbidden", message)
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "Bad Request", message)
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, "Bad Gateway", message)
    }

    fn too_many_requests(retry_after: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            error: "Too Many Requests".into(),
            message: format!("Rate limit exceeded. Retry after {}s", retry_after),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            message,
        )
    }

    fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            error: self.error,
            message: self.message,
            status: self.status.as_u16(),
        };
        (self.status, Json(body)).into_response()
    }
}

// ── Rate Limiter ───────────────────────────────────────────────────────────

struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,
    rps: f64,
    burst: u32,
}

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(rps: f64, burst: u32) -> Self {
        Self {
            buckets: DashMap::new(),
            rps,
            burst,
        }
    }

    fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut entry = self.buckets.entry(key.to_string()).or_insert(TokenBucket {
            tokens: self.burst as f64,
            last_refill: now,
        });

        let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
        entry.tokens = (entry.tokens + elapsed * self.rps).min(self.burst as f64);
        entry.last_refill = now;

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ── Application State ──────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    jwt_secret: String,
    http_client: reqwest::Client,
    neutron_url: String,
    rate_limiter: Arc<RateLimiter>,
    nats_url: String,
}

// ── Main ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize observability
    spectre_observability::init("spectre-proxy")?;
    info!("Starting Spectre Proxy...");

    // 2. Load critical secrets
    let jwt_secret = SecretManager::get("JWT_SECRET")
        .expect("CRITICAL: JWT_SECRET must be set")
        .expose_secret()
        .clone();

    // 3. Build shared HTTP client (reused across requests)
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(20)
        .build()?;

    // 4. Rate limiting configuration
    let rps: f64 = std::env::var("RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100.0);
    let burst: u32 = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);

    let neutron_url = std::env::var("NEUTRON_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string());
    let nats_url = std::env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://localhost:4222".to_string());

    let state = AppState {
        jwt_secret,
        http_client,
        neutron_url,
        rate_limiter: Arc::new(RateLimiter::new(rps, burst)),
        nats_url,
    };

    // 5. Build router
    // Public routes (no auth)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/metrics", get(metrics_endpoint));

    // Protected API routes (with auth + RBAC + rate limiting)
    let api_routes = Router::new()
        .route("/api/v1/ingest", post(ingest_event))
        .route(
            "/api/v1/neutron/*path",
            post(proxy_to_neutron).get(proxy_to_neutron),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    // 6. Bind with optional TLS
    let tls_enabled = std::env::var("TLS_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    if tls_enabled {
        // TODO: Implement TLS support using axum-server with tls-rustls feature
        // Current implementation has type compatibility issues between tower and hyper services
        warn!("TLS is enabled but not yet implemented. Falling back to HTTP.");

        info!("Spectre Proxy listening on {} (TLS disabled - not implemented)", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(spectre_core::shutdown_signal())
        .await?;
    } else {
        let env = std::env::var("SPECTRE_ENV").unwrap_or_else(|_| "dev".to_string());
        if env != "dev" {
            warn!("TLS is disabled outside of development environment. Set TLS_ENABLED=true for production.");
        }

        info!("Spectre Proxy listening on {} (TLS disabled)", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(spectre_core::shutdown_signal())
        .await?;
    }

    // Flush observability on shutdown
    spectre_observability::shutdown();
    info!("Spectre Proxy shut down gracefully");

    Ok(())
}

// ── Middleware: Auth ────────────────────────────────────────────────────────

async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::unauthorized("Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(ApiError::unauthorized("Invalid Authorization header format"));
    }
    let token = &auth_header[7..];

    let key = DecodingKey::from_secret(state.jwt_secret.as_bytes());
    let validation = Validation::default();

    match decode::<Claims>(token, &key, &validation) {
        Ok(token_data) => {
            let claims = token_data.claims;
            info!(sub = %claims.sub, role = %claims.role, "Authenticated");

            // RBAC: Check role against route requirements
            let role = Role::from_str(&claims.role)
                .ok_or_else(|| ApiError::forbidden(format!("Unknown role: {}", claims.role)))?;

            let path = req.uri().path().to_string();
            let required_role = required_role_for_path(&path);

            if role < required_role {
                return Err(ApiError::forbidden(format!(
                    "Role '{}' insufficient for {}. Requires '{:?}' or higher.",
                    claims.role, path, required_role
                )));
            }

            req.extensions_mut().insert(claims);
            Ok(next.run(req).await)
        }
        Err(e) => {
            warn!("JWT validation failed: {}", e);
            Err(ApiError::unauthorized("Invalid or expired token"))
        }
    }
}

/// Determine required role for a given path
fn required_role_for_path(path: &str) -> Role {
    if path.starts_with("/api/v1/admin") {
        Role::Admin
    } else if path.starts_with("/api/v1/ingest") || path.starts_with("/api/v1/neutron") {
        Role::Service
    } else {
        Role::Readonly
    }
}

// ── Middleware: Rate Limiting ───────────────────────────────────────────────

async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    // Key by client IP or "unknown"
    let key = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if !state.rate_limiter.check(&key) {
        let retry_after = 1;
        let mut response = ApiError::too_many_requests(retry_after).into_response();
        response.headers_mut().insert(
            "Retry-After",
            retry_after.to_string().parse().unwrap(),
        );
        return Ok(response);
    }

    Ok(next.run(req).await)
}

// ── Handlers: Public ───────────────────────────────────────────────────────

async fn health_check() -> &'static str {
    "OK"
}

async fn readiness_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    // Check NATS connectivity
    let nats_ok = match spectre_events::EventBus::connect(&state.nats_url).await {
        Ok(bus) => bus.is_connected(),
        Err(_) => false,
    };

    // Check upstream
    let upstream_ok = state
        .http_client
        .get(format!("{}/health", state.neutron_url))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if !nats_ok {
        return Err(ApiError::service_unavailable("NATS is not connected"));
    }

    Ok(Json(serde_json::json!({
        "status": "ready",
        "nats": nats_ok,
        "upstream": upstream_ok,
    })))
}

async fn metrics_endpoint() -> impl IntoResponse {
    let body = spectre_observability::gather_metrics();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

// ── Handlers: Protected ────────────────────────────────────────────────────

async fn ingest_event() -> Json<serde_json::Value> {
    spectre_observability::metrics::record_event_published();
    Json(serde_json::json!({"status": "ingested"}))
}

async fn proxy_to_neutron(
    State(state): State<AppState>,
    Path(path): Path<String>,
    req: Request<Body>,
) -> Result<impl IntoResponse, ApiError> {
    let timer = spectre_observability::metrics::start_request_timer();
    let neutron_url = format!("{}/api/v1/{}", state.neutron_url, path);

    info!("Proxying to Neutron: {}", neutron_url);

    let body_bytes = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| {
            error!("Failed to read body: {}", e);
            ApiError::bad_request(format!("Failed to read request body: {}", e))
        })?;

    let res = state
        .http_client
        .post(&neutron_url)
        .body(body_bytes)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| {
            error!("Neutron upstream error: {}", e);
            ApiError::bad_gateway(format!("Upstream error: {}", e))
        })?;

    let status =
        StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let res_bytes = res
        .bytes()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to read upstream response: {}", e)))?;

    timer.observe_duration();
    spectre_observability::metrics::record_request("POST", &format!("/api/v1/neutron/{}", path), status.as_u16());

    Ok((status, res_bytes))
}

// ── TLS Helpers ────────────────────────────────────────────────────────────

/// Load TLS certificates from PEM file
fn load_certs(path: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let cert_file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open cert file {}: {}", path, e))?;
    let mut reader = std::io::BufReader::new(cert_file);

    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to parse certificates: {}", e))?;

    if certs.is_empty() {
        return Err(anyhow::anyhow!("No certificates found in {}", path));
    }

    Ok(certs)
}

/// Load TLS private key from PEM file
fn load_key(path: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let key_file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open key file {}: {}", path, e))?;
    let mut reader = std::io::BufReader::new(key_file);

    // Try reading as PKCS8 first, then RSA, then EC
    loop {
        match rustls_pemfile::read_one(&mut reader)
            .map_err(|e| anyhow::anyhow!("Failed to read key: {}", e))? {
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(key.into()),
            None => break,
            _ => continue,
        }
    }

    Err(anyhow::anyhow!("No valid private key found in {}", path))
}
