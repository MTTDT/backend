use std::{collections::HashMap, sync::Arc};

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::stocks_register::StocksRegister;

pub type SessionRegister = Arc<Mutex<StocksRegister>>;

fn default_register() -> SessionRegister {
    Arc::new(Mutex::new(StocksRegister::new()))
}


pub type SessionMap = Arc<Mutex<HashMap<String, SessionRegister>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub sessions: SessionMap,
    pub jwt_secret: String,
    pub default_tickers: Vec<DefaultTicker>,
}

#[derive(Clone)]
pub struct DefaultTicker {
    pub symbol: String,
    pub interval: String,
    pub range: String,
}

impl AppState {
    pub fn new(db: SqlitePool, jwt_secret: String) -> Self {
        let default_tickers = vec![
            DefaultTicker { symbol: "AAPL".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "TSLA".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "AMZN".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "NVDA".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "META".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "MSFT".into(), interval: "1d".into(), range: "3mo".into() },
            DefaultTicker { symbol: "GOOGL".into(), interval: "1d".into(), range: "3mo".into() },
        ];

        Self {
            db,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            jwt_secret,
            default_tickers,
        }
    }
    pub async fn get_or_create_session(&self, session_id: &str) -> SessionRegister {
        let mut map = self.sessions.lock().await;
        if let Some(existing) = map.get(session_id) {
            return existing.clone();
        }

        let register = default_register();
        map.insert(session_id.to_owned(), register.clone());

        let reg_clone = register.clone();
        let defaults = self.default_tickers.clone();
        tokio::spawn(async move {
            let mut reg = reg_clone.lock().await;
            for dt in &defaults {
                if let Err(e) = reg.fetch(&dt.symbol, &dt.interval, &dt.range).await {
                    tracing::warn!("Failed to seed {}: {}", dt.symbol, e);
                }
            }
        });

        register
    }

    pub async fn remove_session(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
    }
}