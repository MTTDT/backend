use linfa::prelude::*;
use linfa_linear::LinearRegression;
use ndarray::{Array1, Array2, Axis, s};
use crate::stocks_register::StockRecord;
use serde::{Deserialize, Serialize};


#[derive(Serialize)]
pub struct PredictionResult {
    pub target_ticker: String,
    pub training_data: Vec<f64>,  
    pub actual_test: Vec<f64>,    
    pub predicted_test: Vec<f64>, 
    pub r_squared: f64,
    pub test_timestamps: Vec<i64>, 
    pub train_timestamps: Vec<i64>,
}

pub fn train_and_predict(
    target: &StockRecord, 
    features: Vec<&StockRecord>,
    test_size: f64 
) -> Result<PredictionResult, Box<dyn std::error::Error>> {
    
    let min_len = features.iter()
        .map(|f| f.close_prices.len())
        .min()
        .unwrap_or(0)
        .min(target.close_prices.len());

   

    let n_features = features.len();
    let mut x_matrix = Array2::<f64>::zeros((min_len, n_features));

    for (i, feature_stock) in features.iter().enumerate() {
        let start_idx = feature_stock.close_prices.len() - min_len;
        let prices = &feature_stock.close_prices[start_idx..];
        for (j, &price) in prices.iter().enumerate() {
            x_matrix[[j, i]] = price;
        }
    }

    let target_start = target.close_prices.len() - min_len;
    let y_vector = Array1::from_vec(target.close_prices[target_start..].to_vec());

    let train_len = (min_len as f64 - (min_len as f64 *test_size)) as usize;
    
    let x_train = x_matrix.slice(s![0..train_len, ..]).to_owned();
    let y_train = y_vector.slice(s![0..train_len]).to_owned();
    let x_test = x_matrix.slice(s![train_len..min_len, ..]).to_owned();
    let y_test = y_vector.slice(s![train_len..min_len]).to_owned();

    let dataset = Dataset::new(x_train, y_train);
    let model = LinearRegression::default()
        .with_intercept(true)
        .fit(&dataset)?;

    let predictions = model.predict(&x_test);

    let r2 = predictions.r2(&y_test)?;

    Ok(PredictionResult {
        target_ticker: target.ticker.clone(),
        training_data: y_vector.slice(s![0..train_len]).to_vec(),
        actual_test: y_test.to_vec(),
        predicted_test: predictions.to_vec(),
        r_squared: r2,
        test_timestamps: target.timestamps[train_len..].to_vec(),
        train_timestamps: target.timestamps[0..train_len].to_vec(),
    })
}
