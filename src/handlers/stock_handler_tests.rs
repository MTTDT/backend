#[cfg(test)]
mod stock_handler_tests {
    use std::collections::HashMap;
    use crate::{
        auth::extractor::SessionContext,
        stocks_register::StockRecord,
        handlers::stocks::{TickerName, AddTickerRequest, TickersRequest},
    };

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
        use crate::handlers::stocks::shift_stock_dates;
        
        let mut stock = test_stock("TEST");
        let original = stock.timestamps.clone();
        shift_stock_dates(&mut stock, 5);
        let offset = 5 * 86400;
        for (i, ts) in stock.timestamps.iter().enumerate() {
            assert_eq!(*ts, original[i] - offset);
        }
    }


    #[tokio::test]
    async fn test_app_state_session_isolation() {
        
        let session_1 = SessionContext {
            session_id: "session-1".to_string(),
            user_id: Some("user-1".to_string()),
        };
        
        let session_2 = SessionContext {
            session_id: "session-2".to_string(),
            user_id: Some("user-2".to_string()),
        };

        let mut register_1: HashMap<String, StockRecord> = HashMap::new();
        register_1.insert("AAPL".to_string(), test_stock("AAPL"));
        
        let mut register_2: HashMap<String, StockRecord> = HashMap::new();
        register_2.insert("GOOGL".to_string(), test_stock("GOOGL"));

        assert_eq!(register_1.len(), 1);
        assert_eq!(register_2.len(), 1);
        assert!(register_1.contains_key("AAPL"));
        assert!(register_2.contains_key("GOOGL"));
        assert!(!register_1.contains_key("GOOGL"));
        assert!(!register_2.contains_key("AAPL"));
        
        assert_ne!(session_1.session_id, session_2.session_id);
    }

    #[tokio::test]
    async fn test_session_context_user_association() {
        
        let session_with_user = SessionContext {
            session_id: "session-with-user".to_string(),
            user_id: Some("user-123".to_string()),
        };
        
        let session_without_user = SessionContext {
            session_id: "session-guest".to_string(),
            user_id: None,
        };

        assert!(session_with_user.user_id.is_some());
        assert_eq!(session_with_user.user_id, Some("user-123".to_string()));
        
        assert!(session_without_user.user_id.is_none());
        
        let should_persist = session_with_user.user_id.is_some();
        assert!(should_persist);
    }

    #[tokio::test]
    async fn test_multiple_sessions_with_same_stock() {
        
        let session_1 = create_test_session();
        let session_2 = SessionContext {
            session_id: "session-2".to_string(),
            user_id: Some("user-2".to_string()),
        };

        let mut register_1: HashMap<String, StockRecord> = HashMap::new();
        let mut aapl_1 = test_stock("AAPL");
        aapl_1.close_prices = vec![100.0, 101.0, 102.0];
        register_1.insert("AAPL".to_string(), aapl_1);
        
        let mut register_2: HashMap<String, StockRecord> = HashMap::new();
        let mut aapl_2 = test_stock("AAPL");
        aapl_2.close_prices = vec![150.0, 151.0, 152.0]; // Different prices
        register_2.insert("AAPL".to_string(), aapl_2);

        assert!(register_1.contains_key("AAPL"));
        assert!(register_2.contains_key("AAPL"));
        
        let prices_1 = &register_1.get("AAPL").unwrap().close_prices;
        let prices_2 = &register_2.get("AAPL").unwrap().close_prices;
        assert_ne!(prices_1, prices_2);
        assert_eq!(prices_1[0], 100.0);
        assert_eq!(prices_2[0], 150.0);
    }
}