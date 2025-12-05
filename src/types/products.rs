use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "type")]
pub enum Product {
  Oil {
    id: Uuid,
    product_weight_net: Option<f64>,
    product_weight_gross: Option<f64>,
    volume_oil_at_15: Option<f64>,
  },
  OilProduct {
    id: Uuid,
    product_weight: Option<f64>,
    volume_product_at_15: Option<f64>,
  },
}
impl Default for Product {
  fn default() -> Self {
    Self::Oil {
      id: Uuid::now_v7(),
      product_weight_net: None,
      product_weight_gross: None,
      volume_oil_at_15: None,
    }
  }
}
