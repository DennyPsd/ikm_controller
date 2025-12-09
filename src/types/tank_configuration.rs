use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/** Конфигурация цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankConfig {
  /** Основные данные */
  pub basic_data: TankBasicData,
  /** Уровни точечных датчиков */
  pub levels_of_point_sensors: LevelsOfPointSensors,
  /** Конструкция */
  pub construction: Construction,
  /** Метод расчета массы */
  pub mass_calculation_method: MassCalculationMethod,
  /** Показатели точности измерений */
  pub measurement_accuracy_indicators: MeasurementAccuracyIndicators,
  /** Блок калибровки */
  pub calibration_block: Calibration,
}

/** Основные данные цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankBasicData {
  /** Наименование */
  pub name: Option<String>,
  /** Парк резервуаров */
  pub reservoir_park: Option<String>,
  /** Группа */
  pub group: Option<String>,
  /** Тип цистерны */
  pub tank_type: Option<String>,
  /** Номинальная емкость */
  pub nominal_capacity: Option<f64>,
  /** Плотность хранимой жидкости по данным */
  pub density_stored_liquid_according: Option<f64>,
  /** Базовая высота */
  pub basic_height: Option<f64>,
  /** Продукт */
  pub product: Option<String>,
  /** Максимальный допустимый уровень продукта */
  pub maximum_allowable_product_level: Option<f64>,
}

/** Точечный датчик уровня */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct LevelPointSensor {
  /** Идентификатор */
  pub id: String,
  /** Индекс */
  pub index: i32,
  /** Значение */
  pub value: u32,
}

/** Уровни точечных датчиков */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct LevelsOfPointSensors {
  /** Гистерезис */
  pub hysteresis: Option<f64>,
  /** Точки датчиков */
  #[serde(default)]
  pub points: Vec<LevelPointSensor>,
}

/** Конструкция цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Construction {
  /** Линейное расширение */
  pub linear_expansion: Option<f64>,
  /** Масса плавающего покрытия */
  pub mass_floating_coating: Option<f64>,
}

/** Метод расчета массы */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
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
pub struct MeasurementAccuracyIndicators {
  /** Допустимая абсолютная погрешность */
  pub limit_permissible_absolute_error: Option<f64>,
  /** Предел гидростатического устройства */
  pub hydrostatic_device_limit: Option<f64>,
  /** Предел давления устройства */
  pub pressure_device_limit: Option<f64>,
  /** Допустимая абсолютная погрешность измерения уровня резервуара */
  pub limit_permissible_absolute_measurement_reservoir_level: Option<f64>,
  /** Допустимая абсолютная погрешность измерения уровня сырой воды */
  pub limit_permissible_absolute_measurement_level_raw_water: Option<f64>,
  /** Допустимая абсолютная погрешность измерения температуры продуктов и паров */
  pub limit_permissible_absolute_measurement_temp_products_and_vapours: Option<f64>,
  /** Допустимая абсолютная погрешность измерения плотности нефти */
  pub limit_permissible_absolute_measurement_oil_densities: Option<f64>,
  /** Калибровочная таблица чертежа */
  pub drawing_calibration_table: Option<f64>,
  /** ВПИ гидростатического устройства */
  pub hydrostatic_device_vpi: Option<f64>,
  /** ВПИ давления устройства */
  pub pressure_device_vpi: Option<f64>,
}

/** Калибровка */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Calibration {
  /** Коэффициент уровня */
  pub level_coefficient: Option<f64>,
  /** Точечные датчики уровня */
  pub level_point_sensors: Vec<LevelPointSensor>,
}
