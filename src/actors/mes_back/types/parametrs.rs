use crate::actors::mes_back::types::base::CouchModelExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusLevel {
  #[default]
  Info,
  Warning,
  Alarm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
  pub row: i32,
  pub column: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankGroup {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  #[serde(rename = "parkId")]
  pub park_id: String,

  pub name: String,
  pub position: Position,

  #[serde(default)]
  pub tanks: Vec<String>,
}

impl CouchModelExt for TankGroup {
  fn document_type(&self) -> Option<&'static str> {
    Some("tank_group")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankDataParameter {
  pub value: f64,
  pub unit: String,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub status: Option<StatusLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankData {
  #[serde(default)]
  pub status: StatusLevel,

  pub product_level: TankDataParameter,
  pub min_product_level: TankDataParameter,
  pub max_product_level: TankDataParameter,
  pub weight: TankDataParameter,
  pub volume: TankDataParameter,
  pub product_temperature: TankDataParameter,
  pub density: TankDataParameter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConfigurationBasicData {
  pub name: String,

  #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
  pub tank_type: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub nominal_capacity: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub density_test_fluid: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankConfigurationPointSensorLevel {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  pub name: String,
  pub level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConstruction {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub coefficient_lin_mat_extensions_cladding: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub weight_floating_coating: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConfigurationMethodCalculatingMass {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub p3_p1: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub p1_reference_point: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub reference_point: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConfigurationMeasurementAccuracyIndicators {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error_limit_measuring_distance: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub hydrostatic_pressure_tolerance_limit_led_errors: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub hydrostatic_pressure_vpi: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub vapour_pressure_tolerance_limit_led_errors: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub vapour_pressure_vpi: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub tank_level: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub temperature_products_vapours: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub compilation_hail_tables: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub processing_measurement_result: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConfigurationConnectingMeasuringInstruments {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub level_sensor: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub multi_zone_temperature_sensor: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub hydrostatic_pressure_sensor: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub density_sensor: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub vapour_pressure_sensor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TankConfigurationCalculatedData {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TankConfigurationCalibration {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankConfigurationDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub basic_data: Option<TankConfigurationBasicData>,

  #[serde(rename = "pointSensorlevels", skip_serializing_if = "Option::is_none")]
  pub point_sensorlevels: Option<Vec<TankConfigurationPointSensorLevel>>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub construction: Option<TankConstruction>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub calculated_data: Option<TankConfigurationCalculatedData>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub method_calculating_mass: Option<TankConfigurationMethodCalculatingMass>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub measurement_accuracy_indicators: Option<TankConfigurationMeasurementAccuracyIndicators>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub calibration: Option<TankConfigurationCalibration>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub connecting_measuring_instruments: Option<TankConfigurationConnectingMeasuringInstruments>,
}

impl CouchModelExt for TankConfigurationDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("tank_configuration")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankCalibrationDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub water_level: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub product_temperature: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub vapour_temperature: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub ambient_temperature: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub sample_temperature: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub reference_density: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub observed_density: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub steam_pressure: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub pressure: Option<f64>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub date: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub time: Option<String>,
}

impl CouchModelExt for TankCalibrationDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("tank_calibration")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorTemperature {
  pub name: String,
  pub level: f64,
  pub value: f64,
  pub unit_level: String,
  pub unit_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankParametersDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  pub water_percent: TankDataParameter,
  pub chloride_salts_percent: TankDataParameter,
  pub meh_impurities_percent: TankDataParameter,

  pub max_tank_level: TankDataParameter,
  pub product_level: TankDataParameter,
  pub min_product_level: TankDataParameter,
  pub max_product_level: TankDataParameter,

  pub sensors_temperatures: Vec<SensorTemperature>,

  pub net_product_weight: TankDataParameter,
  pub gross_product_weight: TankDataParameter,
  pub product_volume: TankDataParameter,

  pub water_level: TankDataParameter,
  pub product_temperature: TankDataParameter,
  pub vapour_temperature: TankDataParameter,

  pub product_density: TankDataParameter,
  pub product_density_at15: TankDataParameter,

  pub hydrostatic_pressure: TankDataParameter,
  pub vapour_pressure: TankDataParameter,

  pub capacity_up_max_level: TankDataParameter,
  pub product_up_min_level: TankDataParameter,

  pub expenditure: TankDataParameter,
  pub level_measurement_speed: TankDataParameter,

  pub volume_product_calculated_below_water: TankDataParameter,
  pub volume_raw_water: TankDataParameter,
  pub volume_oil_at15: TankDataParameter,
}

impl CouchModelExt for TankParametersDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("tank_parameters")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TankDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  pub name: String,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub position: Option<Position>,

  #[serde(rename = "productId")]
  pub product_id: String,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<TankData>,

  #[serde(rename = "configurationId", skip_serializing_if = "Option::is_none")]
  pub configuration_id: Option<String>,

  #[serde(rename = "calibrationId", skip_serializing_if = "Option::is_none")]
  pub calibration_id: Option<String>,

  #[serde(rename = "parametersId", skip_serializing_if = "Option::is_none")]
  pub parameters_id: Option<String>,
}

impl CouchModelExt for TankDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("tank")
  }
}
