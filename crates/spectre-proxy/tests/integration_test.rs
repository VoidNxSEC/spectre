use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    role: String,
}

fn generate_token(role: &str, secret: &str) -> String {
    let claims = Claims {
        sub: "test-user".to_string(),
        exp: 10000000000,
        role: role.to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap()
}

// NOTE: These integration tests require the proxy binary to be running separately.
// Run with: JWT_SECRET=secret cargo run -p spectre-proxy
// Then: cargo test -p spectre-proxy --test integration_test -- --ignored

const BASE_URL: &str = "http://127.0.0.1:3000";

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_health_no_auth_required() {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/health", BASE_URL))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_metrics_no_auth_required() {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/metrics", BASE_URL))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.unwrap();
    assert!(body.contains("spectre_proxy_requests_total") || body.contains("# HELP"));
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_auth_rejection_missing_token() {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/ingest", BASE_URL))
        .json(&json!({"data": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Verify JSON error response
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
    assert!(body.get("message").is_some());
    assert!(body.get("status").is_some());
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_auth_rejection_invalid_token() {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/v1/ingest", BASE_URL))
        .header("Authorization", "Bearer invalid-token")
        .json(&json!({"data": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_rbac_readonly_cannot_ingest() {
    let client = reqwest::Client::new();
    let token = generate_token("readonly", "secret");
    let resp = client
        .post(format!("{}/api/v1/ingest", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"data": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_rbac_service_can_ingest() {
    let client = reqwest::Client::new();
    let token = generate_token("service", "secret");
    let resp = client
        .post(format!("{}/api/v1/ingest", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"data": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ingested");
}

#[tokio::test]
#[ignore] // Requires running proxy
async fn test_rbac_admin_can_ingest() {
    let client = reqwest::Client::new();
    let token = generate_token("admin", "secret");
    let resp = client
        .post(format!("{}/api/v1/ingest", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({"data": "test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
