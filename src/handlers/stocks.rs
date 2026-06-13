use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    auth::extractor::SessionContext,
    models::MessageResponse,
    state::AppState,
    stocks_register::StockRecord,
};

#[derive(Deserialize)]
pub struct TickersRequest {
    pub tickers: Vec<String>,
}

#[derive(Serialize)]
pub struct TickerName {
    pub ticker: String,
    pub full_name: String,
}

#[derive(Deserialize, Debug)]
pub struct PredictionRequest {
    pub target: String,
    pub features: Vec<(String, i32)>, //(ticker, date_offset)
    pub test_size: f64,
}

#[derive(Deserialize)]
pub struct AddTickerRequest {
    pub ticker: String,
    pub interval: String,
    pub range: String,
}

// ── /stocks/names ─────────────────────────────────────────────────────────────

pub async fn get_names(
    State(state): State<AppState>,
    session: SessionContext,
) -> Json<Vec<TickerName>> {
    let register = state.get_or_create_session(&session.session_id).await;
    let reg = register.lock().await;
    let names = reg.tickers().iter().map(|ticker| TickerName {
        ticker: ticker.clone(),
        full_name: "no name".to_string(),
    }).collect();
    Json(names)
}

// ── POST /stocks ──────────────────────────────────────────────────────────────

pub async fn get_stocks(
    State(state): State<AppState>,
    session: SessionContext,
    Json(payload): Json<TickersRequest>,
) -> Json<Vec<StockRecord>> {
    let register = state.get_or_create_session(&session.session_id).await;
    let reg = register.lock().await;
    let records = payload.tickers.iter()
        .filter_map(|ticker| reg.get(ticker).cloned())
        .collect();
    Json(records)
}

// ── POST /stocks/add ──────────────────────────────────────────────────────────

pub async fn add_stock(
    State(state): State<AppState>,
    session: SessionContext,
    Json(payload): Json<AddTickerRequest>,
) -> impl IntoResponse {
    let register = state.get_or_create_session(&session.session_id).await;
    let mut reg = register.lock().await;

    match reg.fetch(&payload.ticker, &payload.interval, &payload.range).await {
        Ok(_) => {
            // Persist to DB only for authenticated users
            if let Some(user_id) = session.user_id {
                let ticker = payload.ticker.clone();
                let interval = payload.interval.clone();
                let range = payload.range.clone();
                let db = state.db.clone();
                tokio::spawn(async move {
                    let _ = sqlx::query(
                        "INSERT OR IGNORE INTO user_tickers (user_id, ticker, interval, range)
                         VALUES (?, ?, ?, ?)"
                    )
                    .bind(user_id)
                    .bind(ticker)
                    .bind(interval)
                    .bind(range)
                    .execute(&db)
                    .await;
                });
            }
            (StatusCode::OK, Json(MessageResponse {
                message: format!("Successfully added {}", payload.ticker),
            })).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(MessageResponse {
            message: format!("Failed to add {}: {}", payload.ticker, e),
        })).into_response(),
    }
}

// ── POST /predict ─────────────────────────────────────────────────────────────

pub async fn predict(
    State(state): State<AppState>,
    session: SessionContext,
    Json(payload): Json<PredictionRequest>,
) -> impl IntoResponse {
    println!("Received prediction request: {:?}", payload);
    let register = state.get_or_create_session(&session.session_id).await;

    let (target_stock, feature_stocks) = {
        let reg = register.lock().await;
        let target = match reg.get(&payload.target) {
            Some(s) => s.clone(),
            None => return (StatusCode::NOT_FOUND, Json(MessageResponse {
                message: format!("Target ticker {} not found", payload.target),
            })).into_response(),
        };
        let features: Vec<StockRecord> = payload.features.iter()
            .filter_map(|(ticker, days_offset)| {
                reg.get(ticker).cloned().map(|mut stock| {
                    shift_stock_dates(&mut stock, *days_offset);
                    stock
                })
            })
            .collect();
        (target, features)
    };

    if feature_stocks.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(MessageResponse {
            message: "No valid training tickers provided".into(),
        })).into_response();
    }

    let res = tokio::task::spawn_blocking(move || {
        let feature_refs: Vec<&StockRecord> = feature_stocks.iter().collect();
        match crate::regression_model::train_and_predict(&target_stock, feature_refs, payload.test_size) {
            Ok(prediction) => Json(prediction).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(MessageResponse {
                message: format!("Prediction failed: {e}"),
            })).into_response(),
        }
    }).await;

    match res {
        Ok(r) => r,
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(MessageResponse {
            message: format!("Worker thread error: {e}"),
        })).into_response(),
    }
}

fn shift_stock_dates(stock: &mut StockRecord, days_offset: i32) {
    let offset_seconds = (days_offset as i64) * 86400; 
    for timestamp in &mut stock.timestamps {
        *timestamp -= offset_seconds;
    }
}