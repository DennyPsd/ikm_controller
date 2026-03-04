use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::components::data::DataModel;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "vapor_density_method")]
pub enum VaporDensity {
  /// Ручной ввод плотности паров, кг/м3
  #[serde(rename = "manual")]
  Manual { vapor_density: Option<f64> },

  /// Расчёт по температуре начала кипения, °C
  #[serde(rename = "calculated")]
  Calculated { initial_boiling_point: Option<f64> },
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "type")]
pub enum Product {
  Oil {
    id: Uuid,
    title: SmolStr,
    product_weight: Option<f64>,
    volume_at_15: Option<f64>,

    #[serde(flatten)]
    vapor: VaporDensity,
  },
  OilProduct {
    id: Uuid,
    title: SmolStr,
    product_weight: Option<f64>,
    volume_at_15: Option<f64>,

    #[serde(flatten)]
    vapor: VaporDensity,
  },
}

impl Default for Product {
  fn default() -> Self {
    Self::Oil {
      id: Uuid::now_v7(),
      title: SmolStr::new("default oil"),
      product_weight: None,
      volume_at_15: None,
      vapor: VaporDensity::Manual {
        vapor_density: None,
      },
    }
  }
}

impl DataModel for Product {
  fn id(&self) -> &Uuid {
    match self {
      Product::Oil { id, .. } => id,
      Product::OilProduct { id, .. } => id,
    }
  }

  fn title(&self) -> Option<&SmolStr> {
    match self {
      Product::Oil { title, .. } => Some(title),
      Product::OilProduct { title, .. } => Some(title),
    }
  }
}
