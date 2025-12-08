use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankConfig {
  pub basic_data: TankBasicData,
  pub levels_of_point_sensors: LevelsOfPointSensors,
  pub construction: Construction,
  pub mass_calculation_method: MassCalculationMethod,
  pub measurement_accuracy_indicators: MeasurementAccuracyIndicators,
  pub calibration_block: Calibration,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankBasicData {
  pub name: Option<String>,
  pub reservoir_park: Option<String>,
  pub group: Option<String>,
  pub tank_type: Option<String>,
  pub nominal_capacity: Option<String>,
  pub density_stored_liquid_according: Option<String>,
  pub basic_height: Option<String>,
  pub product: Option<String>,
  pub maximum_allowable_product_level: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct LevelPointSensor {
  pub id: String,
  pub index: i32,
  pub value: u32,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct LevelsOfPointSensors {
  pub hysteresis: Option<String>,
  #[serde(default)]
  pub points: Vec<LevelPointSensor>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Construction {
  pub linear_expansion: Option<String>,
  pub mass_floating_coating: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct MassCalculationMethod {
  pub method: Option<String>,
  pub switching_level: Option<String>,
  pub switching_level_hysteresis: Option<String>,
  pub p3_p1: Option<String>,
  pub p1_reference_point: Option<String>,
  pub reference_point: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct MeasurementAccuracyIndicators {
  pub limit_permissible_absolute_error: Option<String>,
  pub hydrostatic_device_limit: Option<String>,
  pub pressure_device_limit: Option<String>,
  pub limit_permissible_absolute_measurement_reservoir_level: Option<String>,
  pub limit_permissible_absolute_measurement_level_raw_water: Option<String>,
  pub limit_permissible_absolute_measurement_temp_products_and_vapours: Option<String>,
  pub limit_permissible_absolute_measurement_oil_densities: Option<String>,
  pub drawing_calibration_table: Option<String>,
  pub hydrostatic_device_vpi: Option<String>,
  pub pressure_device_vpi: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Calibration {
  pub level_coefficient: Option<String>,
  pub level_point_sensors: Vec<LevelPointSensor>,
}
