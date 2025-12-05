use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProductType {
  #[serde(rename = "oil")]
  Oil {
    id: Uuid,
    product_weight_net: Option<f64>,
    product_weight_gross: Option<f64>,
    volume_oil_at_15: Option<f64>,
  },
  #[serde(rename = "oil-product")]
  OilProduct {
    id: Uuid,
    product_weight: Option<f64>,
    volume_product_at_15: Option<f64>,
  },
}
