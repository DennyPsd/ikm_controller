use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankConfiguration {
  pub basic_data: TankBasicData,
  pub levels_of_point_sensors: LevelsOfPointSensors,
  pub construction: Construction,
  pub mass_calculation_method: MassCalculationMethod,
  pub measurement_accuracy_indicators: MeasurementAccuracyIndicators,
  pub calibration_block: Calibration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelPointSensor {
  pub id: String,
  pub index: i32,
  pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsOfPointSensors {
  pub hysteresis: Option<String>,
  #[serde(default)]
  pub points: Vec<LevelPointSensor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Construction {
  pub linear_expansion: Option<String>,
  pub mass_floating_coating: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassCalculationMethod {
  /// method: SelectItemT | null
  pub method: Option<String>,

  /// switchingLevel: string | null
  #[serde(rename = "switchingLevel")]
  pub switching_level: Option<String>,

  /// switchingLevelHysteresis: string | null
  #[serde(rename = "switchingLevelHysteresis")]
  pub switching_level_hysteresis: Option<String>,

  /// p3_p1: string | null
  #[serde(rename = "p3_p1")]
  pub p3_p1: Option<String>,

  /// p1_referencePoint: string | null
  #[serde(rename = "p1_referencePoint")]
  pub p1_reference_point: Option<String>,

  /// referencePoint: string | null
  #[serde(rename = "referencePoint")]
  pub reference_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementAccuracyIndicators {
  #[serde(rename = "limitPermissibleAbsoluteError")]
  pub limit_permissible_absolute_error: Option<String>,

  #[serde(rename = "hydrostaticDeviceLimit")]
  pub hydrostatic_device_limit: Option<String>,

  #[serde(rename = "pressureDeviceLimit")]
  pub pressure_device_limit: Option<String>,

  #[serde(rename = "limitPermissibleAbsoluteMeasurementReservoirLevel")]
  pub limit_permissible_absolute_measurement_reservoir_level: Option<String>,

  #[serde(rename = "limitPermissibleAbsoluteMeasurementLevelRawWater")]
  pub limit_permissible_absolute_measurement_level_raw_water: Option<String>,

  #[serde(rename = "limitPermissibleAbsoluteMeasurementTempProductsAndVapours")]
  pub limit_permissible_absolute_measurement_temp_products_and_vapours: Option<String>,

  #[serde(rename = "limitPermissibleAbsoluteMeasurementOilDensities")]
  pub limit_permissible_absolute_measurement_oil_densities: Option<String>,

  #[serde(rename = "drawingCalibrationTable")]
  pub drawing_calibration_table: Option<String>,

  #[serde(rename = "hydrostaticDeviceVPI")]
  pub hydrostatic_device_vpi: Option<String>,

  #[serde(rename = "pressureDeviceVPI")]
  pub pressure_device_vpi: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibration {
  #[serde(rename = "levelCoefficient")]
  pub level_coefficient: Option<String>,

  #[serde(rename = "levelPointSensors", default)]
  pub level_point_sensors: Vec<LevelPointSensor>,
}
