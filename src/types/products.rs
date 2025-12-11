use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::data::SharedData;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "type")]
pub enum Product {
  Oil {
    id: Uuid,
    name: SmolStr,
    product_weight_net: Option<f64>,
    product_weight_gross: Option<f64>,
    volume_at_15: Option<f64>,
  },
  OilProduct {
    id: Uuid,
    name: SmolStr,
    product_weight: Option<f64>,
    volume_at_15: Option<f64>,
  },
}
impl Default for Product {
  fn default() -> Self {
    Self::Oil {
      id: Uuid::now_v7(),
      name: SmolStr::new("default oil"),
      product_weight_net: None,
      product_weight_gross: None,
      volume_at_15: None,
    }
  }
}

impl SharedData for Product {
  fn id(&self) -> &Uuid {
    match self {
      Product::Oil { id, .. } => id,
      Product::OilProduct { id, .. } => id,
    }
  }

  fn title(&self) -> Option<&SmolStr> {
    match self {
      Product::Oil { name, .. } => Some(name),
      Product::OilProduct { name, .. } => Some(name),
    }
  }
}
