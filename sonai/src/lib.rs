#![deny(clippy::all)]

use std::sync::LazyLock;

use linfa_clustering::KMeans;
use linfa_preprocessing::linear_scaling::LinearScaler;
use linfa::traits::Transformer;
use sonai_metrics::{
    DistanceFunction, TextMetricFactory, TextMetrics, features_from_metrics, point_confidence,
};

const AI_CLUSTER: usize =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/model.ai.cluster"))[0] as usize;

static MODEL: LazyLock<KMeans<f64, DistanceFunction>> = LazyLock::new(|| {
    let config = bincode::config::standard();
    bincode::serde::decode_from_slice(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/model.kmeans")),
        config,
    )
    .unwrap()
    .0
});


static SCALER: LazyLock<LinearScaler<f64>> = LazyLock::new(|| {
    let config = bincode::config::standard();
    bincode::serde::decode_from_slice(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/model.scaler")),
        config,
    )
    .unwrap()
    .0
});

static METRICS: LazyLock<TextMetricFactory> = LazyLock::new(|| TextMetricFactory::new().unwrap());

#[derive(Debug, serde::Serialize)]
pub struct Prediction {
    pub chance_ai: f64,
    pub chance_human: f64,
    pub metrics: TextMetrics,
}


fn _predict(devlog: &str) -> Prediction {
    let sample = METRICS.calculate(devlog);

    let features = features_from_metrics(&[&sample]); // Array2<f64> of shape (1, n_features)

    let scaled_features = SCALER.transform(features); // still (1, n_features)

    let features_row = scaled_features.row(0);

    let model = &*MODEL;
    let (_, sims) = point_confidence(model, features_row);

    let chance_ai = sims.get(AI_CLUSTER).cloned().unwrap_or(0.0) * 100.0;
    let chance_human = 100.0 - chance_ai;

    Prediction {
        metrics: sample,
        chance_ai,
        chance_human,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn predict(devlog: &str) -> Prediction {
    _predict(devlog)
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn predict(devlog: &str) -> JsValue {
    serde_wasm_bindgen::to_value(&_predict(devlog)).unwrap()
}
