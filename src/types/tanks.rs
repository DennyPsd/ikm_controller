use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::facility::SharedData;
use uuid::Uuid;

/// Тип статуса цистерны
pub type TankStatus = String;

/// Базовые показатели цистерны
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct BaseVars {
  /// Масса
  pub weight: f64,
  /// Рабочий расчетный объем
  pub work_calc_vol: f64,
  /// Средняя температура продукта
  pub product_avg_temp: f64,
  /// Плотность продукта
  pub product_dens: f64,
  /// Текущий уровень
  pub product_level: f64,
}
/// Параметры отображения цистерны
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankDisplay {
  /// Минимальный уровень цистерны
  pub min_tank_level: f64,
  /// Максимальный уровень цистерны
  pub max_tank_level: f64,
  /// Минимальный уровень
  pub min_level: Option<f64>,
  /// Максимальный уровень
  pub max_level: Option<f64>,
}
/// Цистерна
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Tank {
  /// Идентификатор
  pub id: Uuid,
  /// Группа
  pub group: SmolStr,
  /// Продукт
  pub product: Option<Product>,
  /// Наименование
  pub name: SmolStr,
  /// Статус
  pub status: TankStatus,
  /// ГРК (группа резервуарного комплекса)
  pub grc: SmolStr,
  /// ТРК (тип резервуарного комплекса)
  pub trc: SmolStr,
  /// Базовые переменные
  pub base_vars: Option<BaseVars>,
  /// Внешние переменные
  pub ext_vars: Option<ExtVars>,
  /// Конфигурация
  pub config: Option<TankConfig>,
  /// Параметры отображения
  pub display_params: Option<TankDisplay>,
}
impl SharedData for Tank {
  fn id(&self) -> &Uuid {
    &self.id
  }
  fn title(&self) -> Option<&SmolStr> {
    Some(&self.name)
  }
}
/// Расширенные показатели цистерны
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ExtVars {
  /// Объем продукта
  pub product_volume: Option<f64>,
  /// Уровень продукта
  pub product_level: Option<f64>,
  /// Уровень воды
  pub water_level: Option<f64>,
  /// Температура продукта
  pub product_temperature: Option<f64>,
  /// Температура пара
  pub vapour_temperature: Option<f64>,
  /// Плотность продукта
  pub product_density: Option<f64>,
  /// Плотность продукта при 15°C
  pub product_at_15_density: Option<f64>,
  /// Гидростатическое давление
  pub hydrostatic_pressure: Option<f64>,
  /// Давление пара
  pub vapour_pressure: Option<f64>,
  /// Максимальная резервная емкость сверху
  pub reserve_capacity_up_max: Option<f64>,
  /// Минимальный резервный продукт сверху
  pub reserve_product_up_min: Option<f64>,
  /// Потребление при движении продукта
  pub product_movement_consumption: Option<f64>,
  /// Скорость измерения уровня при движении продукта
  pub product_movement_level_measurement_speed: Option<f64>,
  /// Объем продукта под водой
  pub volume_product_calc_below_water: Option<f64>,
  /// Объем сырой воды
  pub volume_raw_water: Option<f64>,
  /// Температуры
  pub temperatures: Vec<Temperature>,

  /// Объем по градуировочной таблице V(гр), м³ (capacity_at_current_level)
  pub volume_coarse: Option<f64>,
  /// Предел относительной погрешности объема δV, %
  pub volume_relative_error_limit: Option<f64>,
  /// Предел относительной погрешности массы брутто δM, %
  pub gross_mass_relative_error_limit: Option<f64>,
}

/// Температурная точка
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct Temperature {
  /// Значение температуры
  pub value: f64,
  /// Уровень
  pub level: u32,
  /// Наименование
  pub name: String,
}
