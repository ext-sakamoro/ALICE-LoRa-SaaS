// SPDX-License-Identifier: AGPL-3.0-or-later
// ALICE-LoRa-SaaS core-engine: LoRaWAN platform

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Default)]
struct Stats {
    joins: u64,
    uplinks: u64,
    device_queries: u64,
    adr_updates: u64,
}

type AppState = Arc<Mutex<Stats>>;

#[derive(Deserialize)]
struct JoinRequest {
    dev_eui: String,
    app_eui: String,
    app_key: String,
}

#[derive(Deserialize)]
struct SendRequest {
    dev_eui: String,
    payload: String,
    port: Option<u8>,
    confirmed: Option<bool>,
}

#[derive(Deserialize)]
struct AdrRequest {
    dev_eui: String,
    dr: Option<u8>,
    tx_power: Option<i8>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "alice-lora-core",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn join(
    State(state): State<AppState>,
    Json(req): Json<JoinRequest>,
) -> (StatusCode, Json<Value>) {
    let session_id = Uuid::new_v4().to_string();
    {
        let mut s = state.lock().unwrap();
        s.joins += 1;
    }
    info!(session_id = %session_id, dev_eui = %req.dev_eui, "LoRaWAN OTAA join accepted");
    (
        StatusCode::OK,
        Json(json!({
            "session_id": session_id,
            "dev_eui": req.dev_eui,
            "app_eui": req.app_eui,
            "join_type": "OTAA",
            "dev_addr": "26011BDA",
            "nwk_s_key": "2B7E151628AED2A6ABF7158809CF4F3C",
            "app_s_key": "2B7E151628AED2A6ABF7158809CF4F3D",
            "status": "accepted",
        })),
    )
}

async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> (StatusCode, Json<Value>) {
    let frame_id = Uuid::new_v4().to_string();
    let port = req.port.unwrap_or(1);
    let confirmed = req.confirmed.unwrap_or(false);
    {
        let mut s = state.lock().unwrap();
        s.uplinks += 1;
    }
    info!(frame_id = %frame_id, dev_eui = %req.dev_eui, port = port, "LoRaWAN uplink sent");
    (
        StatusCode::OK,
        Json(json!({
            "frame_id": frame_id,
            "dev_eui": req.dev_eui,
            "port": port,
            "confirmed": confirmed,
            "fcnt": 42,
            "payload_size": req.payload.len(),
            "status": "queued",
            "dr": 3,
            "freq_hz": 868100000,
        })),
    )
}

async fn list_devices(State(state): State<AppState>) -> Json<Value> {
    {
        let mut s = state.lock().unwrap();
        s.device_queries += 1;
    }
    Json(json!({
        "devices": [
            { "dev_eui": "0102030405060708", "name": "sensor-01", "status": "active", "last_seen": "2026-03-09T00:00:00Z" },
            { "dev_eui": "0807060504030201", "name": "sensor-02", "status": "active", "last_seen": "2026-03-09T01:00:00Z" },
        ],
        "total": 2,
    }))
}

async fn adr(
    State(state): State<AppState>,
    Json(req): Json<AdrRequest>,
) -> (StatusCode, Json<Value>) {
    let dr = req.dr.unwrap_or(3);
    let tx_power = req.tx_power.unwrap_or(14);
    {
        let mut s = state.lock().unwrap();
        s.adr_updates += 1;
    }
    info!(dev_eui = %req.dev_eui, dr = dr, tx_power = tx_power, "ADR configuration updated");
    (
        StatusCode::OK,
        Json(json!({
            "dev_eui": req.dev_eui,
            "dr": dr,
            "tx_power_dbm": tx_power,
            "nb_trans": 1,
            "status": "applied",
        })),
    )
}

async fn stats(State(state): State<AppState>) -> Json<Value> {
    let s = state.lock().unwrap();
    Json(json!({
        "joins": s.joins,
        "uplinks": s.uplinks,
        "device_queries": s.device_queries,
        "adr_updates": s.adr_updates,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let state: AppState = Arc::new(Mutex::new(Stats::default()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/lora/join", post(join))
        .route("/api/v1/lora/send", post(send))
        .route("/api/v1/lora/devices", get(list_devices))
        .route("/api/v1/lora/adr", post(adr))
        .route("/api/v1/lora/stats", get(stats))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8139".to_string());
    let addr = format!("0.0.0.0:{}", port);
    info!("alice-lora-core listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
