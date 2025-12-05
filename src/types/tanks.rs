use crate::types::products::Product;
use crate::types::tank_configuration::TankConfiguration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type TankStatus = String;

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct BaseVars {
  pub weight: f64,
  pub work_calc_vol: f64,
  pub product_avg_temp: f64,
  pub product_dens: f64,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankDisplay {
  pub min_tank_level: f64,
  pub max_tank_level: f64,
  pub min_level: Option<f64>,
  pub max_level: Option<f64>,
  pub level: f64,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Tank {
  pub id: String,
  pub group: String,
  pub product: Product,
  pub name: String,
  pub status: TankStatus,
  pub grc: String,
  pub trc: String,
  pub base_vars: Option<BaseVars>,
  pub ext_vars: Option<ExtVars>,
  pub display_params: Option<TankDisplay>,
  pub configuration: Option<TankConfiguration>,
}
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ExtVars {
  pub product_volume: Option<f64>,
  pub product_level: Option<f64>,
  pub water_level: Option<f64>,
  pub product_temperature: Option<f64>,
  pub vapour_temperature: Option<f64>,
  pub product_density: Option<f64>,
  pub product_at_15_density: Option<f64>,
  pub hydrostatic_pressure: Option<f64>,
  pub vapour_pressure: Option<f64>,
  pub reserve_capacity_up_max: Option<f64>,
  pub reserve_product_up_min: Option<f64>,
  pub product_movement_consumption: Option<f64>,
  pub product_movement_level_measurement_speed: Option<f64>,
  pub volume_product_calc_below_water: Option<f64>,
  pub volume_raw_water: Option<f64>,
  pub temperatures: Vec<Temperature>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Temperature {
  pub value: f64,
  pub level: f64,
  pub name: String,
}
