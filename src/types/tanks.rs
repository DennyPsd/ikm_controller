use crate::types::parametrs::MainParameters;
use crate::types::products::ProductType;
use crate::types::tank_configuration::TankConfiguration;
use serde::{Deserialize, Serialize};

pub type TankStatus = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankParameters {
  pub weight: f64,
  pub work_calc_vol: f64,
  pub product_avg_temp: f64,
  pub product_dens: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankDisplay {
  pub min_tank_level: f64,
  pub max_tank_level: f64,
  pub min_level: Option<f64>,
  pub max_level: Option<f64>,
  pub level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tank {
  pub id: String,
  pub group: String,
  pub product: ProductType,
  pub parameters: Parameters,
  pub name: String,
  pub status: TankStatus,
  pub grc: String,
  pub trc: String,
  pub configuration: TankConfiguration,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameters {
  pub tank_parameters: Option<TankParameters>,
  pub tank_display: Option<TankDisplay>,
  pub main_parameters: Option<MainParameters>,
}
