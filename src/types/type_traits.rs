use std::collections::HashMap;

use ikm_calc::calculation::core::{CalculationResult, Constants};
use ikm_calc::calculation::kmh::KMHReport;

use crate::actors::tank_calc::{Meta, TimeSeriesEntry};
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Temperature};

/// Отчёт КМХ собирается из трёх источников:
/// 1) Константы конфигурации (`Constants`) через `KMHReportExt::apply_constants`
/// 2) Результат расчёта ядра (`CalculationResult`) через `BaseVars`/`ExtVars` и
///    `KMHReportExt::apply_base_ext`
/// 3) Ручной ввод оператора при проведении КМХ
///
/// По итогу **оператор заполняет или корректирует вручную** следующие поля:
/// - `air_temperature_outside`  — температура наружного воздуха в момент поверки
/// - `air_pressure_outside`     — атмосферное давление снаружи
/// - `wind_speed`               — скорость ветра
/// - `gas_layer_height_measured_points` — результаты измерения высоты газового пространства по точкам
/// - `measured_height`          — показания измерительной рулетки / контрольного уровня, H, мм
///
/// - `density_verified`         — плотность НП по результатам контрольного измерения (лаборатория/эталон)
/// - `density_measured_controlled` — тройка контрольных плотностей (⍴(в), ⍴(c), ⍴(н))
/// - `air_temp_verify`          — температура воздуха при поверке резервуара t(в)
///
/// - `tape_class`               — класс точности измерительной рулетки
/// - `ruler_alpha_coefficient`  — температурный коэффициент линейного расширения материала рулетки
/// - `pressure_coefficient`     — коэффициент поправки на давление (если применяется методикой)
///
/// - `level_channels`           — результаты проверки канала уровня
/// - `temperature_channels`     — результаты проверки температурных каналов
/// - `mass_channels`            — результаты проверки канала массы
/// - `volume_channels`          — результаты проверки канала объёма
/// - `density_channels`         — результаты проверки канала плотности
///
/// Поля, которые **обычно приходят из системы автоматически**:
/// - из `Constants`:
///   - `pontoon_mass`              — масса понтона
///   - `wall_alpha_coefficient`    — температурный коэффициент материала стенки резервуара
///   - `delta_height`              — предел абсолютной погрешности измерения расстояния, ΔH
///   - `nominal_height`            — базовая/номинальная высота резервуара по градуировке
///
/// - из `BaseVars`/`ExtVars` (через `CalculationResult`):
///   - `density_measured`          — плотность НП по показаниям ИС, ρ(ис)
///   - `product_volume_measured`   — объём НП по показаниям ИС, V(ис)
///   - `product_mass_measured`     — масса НП по показаниям ИС, m(ис)
///   - `vapor_temp`                — температура паров НП в резервуаре, t(а)
///   - `volume_coarse`             — объём по градуировочной таблице, V(гр)
///   - `delta_v_max`               — допустимое отклонение по объёму, δV
///   - `delta_m_max`               — допустимое отклонение по массе, δM
///
/// Если оператор вручную меняет автоматом заполненные поля
/// (`density_measured`, `product_volume_measured`, `product_mass_measured`,
/// `vapor_temp`, `volume_coarse`, `delta_v_max`, `delta_m_max`), эти значения
/// могут быть обратно записаны в `BaseVars`/`ExtVars` через
/// `KMHReportExt::write_to_base_ext` и в `Constants` через `write_to_constants`.
///
///
/// Расширения для KMHReport: обогащение из Constants и BaseVars/ExtVars,
/// запись обратно в Constants, и двусторонний обмен с BaseVars/ExtVars.
#[allow(dead_code)]
pub trait KMHReportExt {
  /// Обогащаем отчёт КМХ данными из Constants (прямой маппинг, без расчётов)
  fn apply_constants(&mut self, c: &Constants);

  /// Обновляем Constants на основе данных из KMHReport (обратный маппинг)
  fn write_to_constants(&self, c: &mut Constants);

  /// Обогащаем отчёт КМХ данными из BaseVars + ExtVars
  /// (идея: не зависеть напрямую от CalculationResult)
  fn apply_base_ext(&mut self, base: &BaseVars, ext: &ExtVars);

  /// Обновляем BaseVars + ExtVars на основе данных из KMHReport
  /// (обратный маппинг: KMHReport → BaseVars/ExtVars)
  fn write_to_base_ext(&self, base: &mut BaseVars, ext: &mut ExtVars);
}

impl KMHReportExt for KMHReport {
  /// Constants → KMHReport
  fn apply_constants(&mut self, c: &Constants) {
    // Масса понтона
    if let Some(pontoon) = c.pontoon_weight {
      self.pontoon_mass = pontoon;
    }

    // Коэф. линейного расширения стенки резервуара
    self.wall_alpha_coefficient = c.tank_wall_alpha;

    // Предел абсолютной погрешности измерения расстояния
    self.delta_height = c.max_level_abs_error;

    // Базовая/номинальная высота резервуара
    self.nominal_height = c.structure_base_height;

    // Плотность по ИС (может быть позже перезаписана данными измерений)
    if let Some(rho) = c.tank_product_density {
      self.density_measured = rho;
    }
  }

  /// KMHReport → Constants
  fn write_to_constants(&self, c: &mut Constants) {
    // Масса понтона (в Constants — Option)
    c.pontoon_weight = Some(self.pontoon_mass);

    // Коэф. линейного расширения стенки резервуара
    c.tank_wall_alpha = self.wall_alpha_coefficient;

    // Предел абсолютной погрешности измерения расстояния
    c.max_level_abs_error = self.delta_height;

    // Базовая/номинальная высота резервуара
    c.structure_base_height = self.nominal_height;

    // Плотность по ИС (в Constants — Option)
    c.tank_product_density = Some(self.density_measured);
  }

  /// BaseVars + ExtVars → KMHReport
  fn apply_base_ext(&mut self, base: &BaseVars, ext: &ExtVars) {
    // Плотность по ИС ρ(ис)
    if let Some(rho) = ext.product_density {
      self.density_measured = rho;
    } else {
      self.density_measured = base.product_dens;
    }

    // Объём по ИС V(ис)
    if let Some(v) = ext.product_volume {
      self.product_volume_measured = v;
    } else {
      self.product_volume_measured = base.work_calc_vol;
    }

    // Масса по ИС m(ис) — берём из BaseVars.weight (брутто)
    self.product_mass_measured = base.weight;

    // Температура паров t(а)
    if let Some(t) = ext.vapour_temperature {
      self.vapor_temp = t;
    }

    // Объём по градуировке V(гр)
    if let Some(vg) = ext.volume_coarse {
      self.volume_coarse = vg;
    }

    // Допустимое отклонение по объёму, %
    if let Some(dv) = ext.volume_relative_error_limit {
      self.delta_v_max = dv;
    }

    // Допустимое отклонение по массе, %
    if let Some(dm) = ext.gross_mass_relative_error_limit {
      self.delta_m_max = dm;
    }

    // Остальные поля KMHReport (воздух, давление, ветер, классы рулеток,
    // каналы уровня/массы/объёма/плотности) остаются на совести других слоёв.
  }

  /// KMHReport → BaseVars + ExtVars
  fn write_to_base_ext(&self, base: &mut BaseVars, ext: &mut ExtVars) {
    // Плотность по ИС ρ(ис)
    base.product_dens = self.density_measured;
    ext.product_density = Some(self.density_measured);

    // Объём по ИС V(ис)
    base.work_calc_vol = self.product_volume_measured;
    ext.product_volume = Some(self.product_volume_measured);

    // Масса по ИС m(ис)
    base.weight = self.product_mass_measured;

    // Температура паров t(а)
    ext.vapour_temperature = Some(self.vapor_temp);

    // Объём по градуировке V(гр)
    ext.volume_coarse = Some(self.volume_coarse);

    // Допустимое отклонение по объёму, %
    ext.volume_relative_error_limit = Some(self.delta_v_max);

    // Допустимое отклонение по массе, %
    ext.gross_mass_relative_error_limit = Some(self.delta_m_max);

    // Остальное (уровни, давления, температуры по точкам и т.п.)
    // KMHReport в себе не держит, поэтому тут не трогаем.
  }
}

/// Экстеншен для CalculationResult: маппинг в наши типы BaseVars / ExtVars
pub trait CalculationResultExt {
  fn to_base_vars(&self) -> BaseVars;

  fn to_ext_vars(&self, meta: &Meta, config: &TankConfig, entry: &TimeSeriesEntry) -> ExtVars;
}

impl CalculationResultExt for CalculationResult {
  /// CalculationResult -> BaseVars (чистый маппинг)
  fn to_base_vars(&self) -> BaseVars {
    BaseVars {
      // Масса – брутто из ядра (тонны)
      // weight: self.gross_product_mass,
      weight: self.gross_product_mass / 1000.0,
      // Рабочий объем – V при рабочих условиях
      work_calc_vol: self.product_volume,
      // Средняя температура продукта
      product_avg_temp: self.product_avg_temperature,
      // Плотность при условиях измерения
      product_dens: self.product_density,
    }
  }

  /// CalculationResult (+ контекст) -> ExtVars
  fn to_ext_vars(&self, meta: &Meta, config: &TankConfig, entry: &TimeSeriesEntry) -> ExtVars {
    // уровни температурных датчиков из конфигурации
    let temperature_levels: HashMap<_, _> = config
      .levels_of_point_sensors
      .points
      .iter()
      .map(|v| (v.id.to_uppercase(), v.clone()))
      .collect();

    let temps_src = vec![
      ("T0", entry.t0),
      ("T1", entry.t1),
      ("T2", entry.t2),
      ("T3", entry.t3),
      ("T4", entry.t4),
      ("T5", entry.t5),
      ("T6", entry.t6),
      ("T7", entry.t7),
      ("T8", entry.t8),
      ("T9", entry.t9),
    ];

    let temperatures = temps_src
      .into_iter()
      .map(|(name, value)| Temperature {
        value,
        name: name.to_string(),
        level: temperature_levels
          .get(name)
          .cloned()
          .unwrap_or_default()
          .value,
      })
      .collect();

    ExtVars {
      product_volume: Some(self.product_volume),
      product_level: Some(self.product_level),
      water_level: Some(entry.h_v),
      product_temperature: Some(self.product_avg_temperature),
      vapour_temperature: Some(self.vapor_avg_temperature),
      product_density: Some(self.product_density),
      product_at_15_density: Some(self.product_density_15),
      hydrostatic_pressure: Some(entry.p1),
      vapour_pressure: Some(entry.p3),
      reserve_capacity_up_max: Some(meta.constants.h_max_level),
      reserve_product_up_min: Some(meta.constants.h_critical_level),
      product_movement_consumption: Some(0.0),
      product_movement_level_measurement_speed: Some(self.velocity_product_level),
      volume_product_calc_below_water: Some(self.water_volume),
      volume_raw_water: Some(self.water_volume),
      temperatures,
      volume_coarse: Some(self.capacity_at_current_level),
      volume_relative_error_limit: Some(self.volume_relative_error_limit),
      gross_mass_relative_error_limit: Some(self.gross_mass_relative_error_limit),
    }
  }
}
