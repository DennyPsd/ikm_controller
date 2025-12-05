use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainParameters {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temperature {
  pub value: f64,
  pub level: f64,
  pub name: String,
}
