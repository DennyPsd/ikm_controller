use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::tanks::TankStatus;
use serde_with::*;

/** Конфигурация цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct TankConfig {
  /** Основные данные */
  pub basic_data: TankBasicData,
  /** Уровни точечных датчиков */
  pub levels_of_point_sensors: LevelsOfPointSensors,
  /** Конструкция */
  pub construction: Construction,
  /** данные заполняются в параметрах резервуара */
  pub parameters: TankParameters,
  /** Метод расчета массы */
  pub mass_calculation_method: MassCalculationMethod,
  /** Показатели точности измерений */
  pub measurement_accuracy_indicators: MeasurementAccuracyIndicators,
  /** Блок калибровки */
  pub calibration_block: Calibration,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct TankParameters {
  /** Наименование */
  pub density_at_15: Option<f32>,
  /** Парк резервуаров */
  pub water_percent: Option<f32>,
  /** Группа */
  pub salts_percent: Option<f32>,
  /** Тип цистерны */
  pub impurities_percent: Option<f32>,

  pub status: TankStatus,
}

/** Основные данные цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct TankBasicData {
  /** Наименование */
  pub title: Option<String>,
  /** Парк резервуаров */
  pub reservoir_park: Option<String>,
  /** Группа */
  pub group: Option<String>,
  /** Тип цистерны */
  pub tank_type: Option<String>,
  /** Номинальная емкость */
  pub nominal_capacity: Option<f32>,
  /** Плотность хранимой жидкости по данным */
  pub density_stored_liquid_according: Option<f32>,
  /** Базовая высота */
  pub basic_height: Option<f32>,
  // /** Продукт */
  pub product: Option<String>,
  /** Максимальный допустимый уровень продукта */
  pub maximum_allowable_product_level: Option<f32>,
  /** Температура воздуха при поверке резервуара */
  pub air_temp_verify: Option<f32>,
}

/** Точечный датчик уровня */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct LevelPointSensor {
  /** Идентификатор */
  pub id: String,
  /** Индекс */
  pub index: i32,
  /** Значение */
  pub value: f32,
}

/** Уровни точечных датчиков */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct LevelsOfPointSensors {
  /** Гистерезис */
  pub hysteresis: Option<f64>,
  /** Точки датчиков */
  #[serde(default)]
  pub points: Vec<LevelPointSensor>,
}

/** Конструкция цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct Construction {
  /** Линейное расширение */
  pub linear_expansion: Option<f32>,
  /** Масса плавающего покрытия */
  pub mass_floating_coating: Option<f32>,
}

/** Метод расчета массы */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct MassCalculationMethod {
  /** Метод */
  pub method: Option<String>,
  /** Уровень переключения */
  pub switching_level: Option<f64>,
  /** Гистерезис уровня переключения */
  pub switching_level_hysteresis: Option<f64>,
  /** P3-P1 */
  pub p3_p1: Option<f64>,
  /** Опорная точка P1 */
  pub p1_reference_point: Option<f64>,
  /** Опорная точка */
  pub reference_point: Option<f64>,
}

/** Показатели точности измерений */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct MeasurementAccuracyIndicators {
  /** Допустимая абсолютная погрешность */
  pub limit_permissible_absolute_error: Option<f32>,
  /** Предел гидростатического устройства */
  pub hydrostatic_device_limit: Option<f32>,
  /** Предел давления устройства */
  pub pressure_device_limit: Option<f32>,
  /** Допустимая абсолютная погрешность измерения уровня резервуара */
  pub limit_permissible_absolute_measurement_reservoir_level: Option<f32>,
  /** Допустимая абсолютная погрешность измерения уровня сырой воды */
  pub limit_permissible_absolute_measurement_level_raw_water: Option<f32>,
  /** Допустимая абсолютная погрешность измерения температуры продуктов и паров */
  pub limit_permissible_absolute_measurement_temp_products_and_vapours: Option<f32>,
  /** Допустимая абсолютная погрешность измерения плотности нефти */
  pub limit_permissible_absolute_measurement_oil_densities: Option<f32>,
  /** Калибровочная таблица чертежа */
  pub drawing_calibration_table: Option<f32>,
  /** ВПИ гидростатического устройства */
  pub hydrostatic_device_vpi: Option<f32>,
  /** ВПИ давления устройства */
  pub pressure_device_vpi: Option<f32>,
}

/** Калибровка */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[skip_serializing_none]
pub struct Calibration {
  /** Коэффициент уровня */
  pub level_coefficient: Option<f64>,
  /** Точечные датчики уровня */
  pub level_point_sensors: Vec<LevelPointSensor>,
}
