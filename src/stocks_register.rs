use std::collections::HashMap;
use yahoo_finance_api as yahoo;
use time::OffsetDateTime;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StockRecord {
    pub ticker: String,
    pub timestamps: Vec<i64>,
    pub close_prices: Vec<f64>,
    pub open_prices: Vec<f64>,
    pub high_prices: Vec<f64>,
    pub low_prices: Vec<f64>,
}

pub struct StocksRegister {
    pub stocks: HashMap<String, StockRecord>,
}

impl StocksRegister {
    pub fn new() -> Self {
        Self {
            stocks: HashMap::new(),
        }
    }

    pub async fn fetch(&mut self, ticker: &str, interval: &str,  range: &str) -> Result<(), Box<dyn std::error::Error>> {
        let provider = yahoo::YahooConnector::new()?;
        let response: yahoo::YResponse = provider.get_quote_range(ticker, interval, range).await?;
        let quotes = response.quotes()?;

        let timestamps = quotes.iter().map(|q| q.timestamp as i64).collect();
        let close_prices = quotes.iter().map(|q| q.close ).collect();
        let open_prices: Vec<f64> = quotes.iter().map(|q| q.open).collect();
        let high_prices: Vec<f64> = quotes.iter().map(|q| q.high).collect();
        let low_prices: Vec<f64> = quotes.iter().map(|q| q.low).collect();

        self.stocks.insert(
            ticker.to_uppercase(),
            StockRecord {
                ticker: ticker.to_uppercase(),
                timestamps,
                close_prices,
                open_prices,
                high_prices,
                low_prices,
            },
        );

        Ok(())
    }

    pub fn get(&self, ticker: &str) -> Option<&StockRecord> {
        println!("Getting stock data for ticker: {:?}", ticker);

        self.stocks.get(&ticker.to_uppercase())
    }

    pub fn all(&self) -> Vec<&StockRecord> {
        self.stocks.values().collect()
    }

    pub fn tickers(&self) -> Vec<String> {
        self.stocks.keys().cloned().collect()
    }

    pub fn remove(&mut self, ticker: &str) -> bool {
        self.stocks.remove(&ticker.to_uppercase()).is_some()
    }

  
}