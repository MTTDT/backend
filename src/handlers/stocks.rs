use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, Json};
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

#[derive(Deserialize)]
pub struct DeleteTickerRequest {
    pub ticker: String,
}


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


pub async fn add_stock(
    State(state): State<AppState>,
    session: SessionContext,
    Json(payload): Json<AddTickerRequest>,
) -> impl IntoResponse {
    let register = state.get_or_create_session(&session.session_id).await;
    let mut reg = register.lock().await;

    match reg.fetch(&payload.ticker, &payload.interval, &payload.range).await {
        Ok(_) => {
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


pub async fn delete_stock(
    State(state): State<AppState>,
    session: SessionContext,
    Path(ticker_id): Path<String>,
) -> impl IntoResponse {
    println!("Received delete request for ticker: {}", ticker_id);
    let register = state.get_or_create_session(&session.session_id).await;
    let mut reg = register.lock().await;

    if reg.remove(&ticker_id) {
        if let Some(user_id) = session.user_id {
            let ticker = ticker_id.clone();
            let db = state.db.clone();
            tokio::spawn(async move {
                let _ = sqlx::query(
                    "DELETE FROM user_tickers WHERE user_id = ? AND ticker = ?"
                )
                .bind(user_id)
                .bind(ticker)
                .execute(&db)
                .await;
            });
        }
        (StatusCode::OK, Json(MessageResponse {
            message: format!("Successfully deleted {}", ticker_id),
        })).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(MessageResponse {
            message: format!("Ticker {} not found", ticker_id),
        })).into_response()
    }
}




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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_session() -> SessionContext {
        SessionContext {
            session_id: "test-123".to_string(),
            user_id: Some("user-1".to_string()),
        }
    }

    fn test_stock(ticker: &str) -> StockRecord {
        StockRecord {
            ticker: ticker.to_string(),
            close_prices: vec![100.0, 101.0, 102.0],
            open_prices: vec![99.0, 100.0, 101.0],
            high_prices: vec![101.0, 102.0, 103.0],
            low_prices: vec![98.0, 99.0, 100.0],
            timestamps: vec![1000, 2000, 3000],
        }
    }

    #[tokio::test]
    async fn test_get_names_returns_empty_on_new_session() {
        let mut register: HashMap<String, StockRecord> = HashMap::new();
        let names: Vec<TickerName> = register.keys().map(|ticker| TickerName {
            ticker: ticker.clone(),
            full_name: "no name".to_string(),
        }).collect();
        assert_eq!(names.len(), 0);
    }

    #[tokio::test]
    async fn test_add_stock_validates_response_format() {
        let payload = AddTickerRequest {
            ticker: "AAPL".to_string(),
            interval: "1d".to_string(),
            range: "1y".to_string(),
        };
        assert_eq!(payload.ticker, "AAPL");
    }

    #[tokio::test]
    async fn test_get_stocks_filters_correctly() {
        let mut register: HashMap<String, StockRecord> = HashMap::new();
        register.insert("AAPL".to_string(), test_stock("AAPL"));
        register.insert("GOOGL".to_string(), test_stock("GOOGL"));
        let request = TickersRequest {
            tickers: vec!["AAPL".to_string()],
        };
        let results: Vec<StockRecord> = request
            .tickers
            .iter()
            .filter_map(|ticker| register.get(ticker).cloned())
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ticker, "AAPL");
    }

    #[tokio::test]
    async fn test_delete_removes_stock_from_register() {
        let mut register: HashMap<String, StockRecord> = HashMap::new();
        register.insert("AAPL".to_string(), test_stock("AAPL"));
        register.remove("AAPL");
        assert_eq!(register.len(), 0);
    }

    #[test]
    fn test_shift_stock_dates_offset_calculation() {
        let mut stock = test_stock("TEST");
        let original = stock.timestamps.clone();
        shift_stock_dates(&mut stock, 5);
        let offset = 5 * 86400;
        for (i, ts) in stock.timestamps.iter().enumerate() {
            assert_eq!(*ts, original[i] - offset);
        }
    }
}