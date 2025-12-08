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
  pub nominal_capacity: Option<String>,
  /** Плотность хранимой жидкости по данным */
  pub density_stored_liquid_according: Option<String>,
  /** Базовая высота */
  pub basic_height: Option<String>,
  /** Продукт */
  pub product: Option<String>,
  /** Максимальный допустимый уровень продукта */
  pub maximum_allowable_product_level: Option<String>,
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
  pub hysteresis: Option<String>,
  /** Точки датчиков */
  #[serde(default)]
  pub points: Vec<LevelPointSensor>,
}

/** Конструкция цистерны */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Construction {
  /** Линейное расширение */
  pub linear_expansion: Option<String>,
  /** Масса плавающего покрытия */
  pub mass_floating_coating: Option<String>,
}

/** Метод расчета массы */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct MassCalculationMethod {
  /** Метод */
  pub method: Option<String>,
  /** Уровень переключения */
  pub switching_level: Option<String>,
  /** Гистерезис уровня переключения */
  pub switching_level_hysteresis: Option<String>,
  /** P3-P1 */
  pub p3_p1: Option<String>,
  /** Опорная точка P1 */
  pub p1_reference_point: Option<String>,
  /** Опорная точка */
  pub reference_point: Option<String>,
}

/** Показатели точности измерений */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct MeasurementAccuracyIndicators {
  /** Допустимая абсолютная погрешность */
  pub limit_permissible_absolute_error: Option<String>,
  /** Предел гидростатического устройства */
  pub hydrostatic_device_limit: Option<String>,
  /** Предел давления устройства */
  pub pressure_device_limit: Option<String>,
  /** Допустимая абсолютная погрешность измерения уровня резервуара */
  pub limit_permissible_absolute_measurement_reservoir_level: Option<String>,
  /** Допустимая абсолютная погрешность измерения уровня сырой воды */
  pub limit_permissible_absolute_measurement_level_raw_water: Option<String>,
  /** Допустимая абсолютная погрешность измерения температуры продуктов и паров */
  pub limit_permissible_absolute_measurement_temp_products_and_vapours: Option<String>,
  /** Допустимая абсолютная погрешность измерения плотности нефти */
  pub limit_permissible_absolute_measurement_oil_densities: Option<String>,
  /** Калибровочная таблица чертежа */
  pub drawing_calibration_table: Option<String>,
  /** ВПИ гидростатического устройства */
  pub hydrostatic_device_vpi: Option<String>,
  /** ВПИ давления устройства */
  pub pressure_device_vpi: Option<String>,
}

/** Калибровка */
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Calibration {
  /** Коэффициент уровня */
  pub level_coefficient: Option<String>,
  /** Точечные датчики уровня */
  pub level_point_sensors: Vec<LevelPointSensor>,
}
