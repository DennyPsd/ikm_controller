use chrono::{DateTime, Utc};
use ikm_calc::calculation::core::{
  Calculation, CalculationMethod, CalculationResult, Constants, ProductType, TemperatureSensor,
  Variables,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use crate::types::{
  tank_configuration::TankConfig,
  tanks::{BaseVars, ExtVars, Temperature},
};

use ikm_calc::calculation::types::GradTableItem;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct LevelCoefficientPoint {
  id: String,
  level_mm: f64,
  kti: f64,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Meta {
  constants: Constants,
  level_coefficient_points: Vec<LevelCoefficientPoint>,
  /// grad_table: [level, volume]
  grad_table: Vec<Vec<f64>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct TimeSeriesEntry {
  ts: i64,
  p1: f64,
  p3: f64,
  h_measured: f64,
  h_v: f64,
  density: f64,
  t0: f64,
  t1: f64,
  t2: f64,
  t3: f64,
  t4: f64,
  t5: f64,
  t6: f64,
  t7: f64,
  t8: f64,
  t9: f64,
}

#[derive(Serialize, Debug)]
struct BaseVarsWithDate {
  date: DateTime<Utc>,
  changed_at: DateTime<Utc>,
  #[serde(flatten)]
  data: BaseVars,
}

#[derive(Serialize, Debug)]
struct ExtVarsWithDate {
  date: DateTime<Utc>,
  changed_at: DateTime<Utc>,
  #[serde(flatten)]
  data: ExtVars,
}

pub struct TankCalcState {
  pub current_indices: HashMap<Uuid, usize>,
  pub previous_results: HashMap<Uuid, CalculationResult>,
}

pub struct TankCalcActor;

impl TankCalcActor {
  pub fn new() -> Self {
    Self
  }
}

#[derive(Debug)]
pub enum TankCalcMsg {
  Tick,
}

#[ractor::async_trait]
impl Actor for TankCalcActor {
  type Msg = TankCalcMsg;
  type State = TankCalcState;
  type Arguments = ();

  async fn pre_start(
    &self,
    myself: ActorRef<Self::Msg>,
    _args: Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    let _ = myself.cast(TankCalcMsg::Tick);
    info!("TankCalc запущен");
    Ok(TankCalcState {
      current_indices: HashMap::new(),
      previous_results: HashMap::new(),
    })
  }

  #[allow(clippy::let_underscore_future)]
  async fn handle(
    &self,
    myself: ActorRef<Self::Msg>,
    msg: TankCalcMsg,
    state: &mut TankCalcState,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      TankCalcMsg::Tick => {
        // Читаем tanks.yaml
        let tanks_yaml = match fs::read_to_string("assets/db/tanks.yaml") {
          Ok(content) => content,
          Err(e) => {
            error!("Ошибка чтения tanks.yaml: {}", e);
            let _ = myself.send_after(Duration::from_secs(10), || TankCalcMsg::Tick);
            return Ok(());
          }
        };

        let tanks: Vec<crate::types::tanks::Tank> = match serde_saphyr::from_str(&tanks_yaml) {
          Ok(t) => t,
          Err(e) => {
            error!("Ошибка разбора tanks.yaml: {}", e);
            let _ = myself.send_after(Duration::from_secs(10), || TankCalcMsg::Tick);
            return Ok(());
          }
        };

        let tank_ids: Vec<Uuid> = tanks.into_iter().map(|t| t.id).collect();

        for tank_id in tank_ids {
          // info!("Расчет для танка {}", tank_id);
          if let Err(e) = self.process_tank(&tank_id, state).await {
            error!("Ошибка ID tank {}: {}", tank_id, e);
          }
        }

        let _ = myself.send_after(Duration::from_secs(2), || TankCalcMsg::Tick);
      }
    }

    Ok(())
  }
}

impl TankCalcActor {
  async fn process_tank(
    &self,
    tank_id: &Uuid,
    state: &mut TankCalcState,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let meta_path = format!("assets/db/calc/{}/meta.json", tank_id);
    let time_series_path = format!("assets/db/calc/{}/time_series.json", tank_id);
    let config_vars_path = format!("assets/db/tanks/{}/config.yaml", tank_id);

    //
    let base_vars_path = format!("assets/db/tanks/{}/base_vars.yaml", tank_id);
    let ext_vars_path = format!("assets/db/tanks/{}/ext_vars.yaml", tank_id);

    // Читаем meta.json
    let meta_content = fs::read_to_string(&meta_path)?;
    let meta: Meta =
      serde_json::from_str(&meta_content).map_err(|err| format!("Cant parse meta: {err:?}"))?;

    // Читаем config.yaml
    let config_content = fs::read_to_string(&config_vars_path)?;
    let config: TankConfig = serde_saphyr::from_str(&config_content)
      .map_err(|err| format!("Cant parse config: {err:?}"))?;

    // Читаем time_series.json
    let time_series_content = fs::read_to_string(&time_series_path)?;
    let time_series: Vec<TimeSeriesEntry> = serde_json::from_str(&time_series_content)
      .map_err(|err| format!("Cant parse time_series:{err:?}"))?;

    if time_series.is_empty() {
      return Ok(());
    }

    // Берем индекс из "ts" в time_series.json, с учетом того, что они могут повторяться
    let current_index = state.current_indices.entry(*tank_id).or_insert(0);
    let next_index = (*current_index + 1) % time_series.len();
    let entry = &time_series[next_index];
    *current_index = next_index;

    let prev_result = state.previous_results.get(tank_id).cloned();

    let calc = self.build_calculation(&meta, &config, entry, prev_result);

    // Запуск расчета ядра
    let result = match calc.calculate() {
      Ok(res) => res,
      Err(e) => {
        error!("Ошибка расчета для {}: {:?}", tank_id, e);
        return Ok(());
      }
    };

    // info!(
    //   "Танк {}: результат расчёта ts={} -> масса={:.2} т, объём={:.2} м³, уровень={:.1} мм, Tср={:.2} °C, ρ={:.4} т/м³",
    //   tank_id,
    //   entry.ts,
    //   result.gross_product_mass,
    //   result.product_volume,
    //   result.product_level,
    //   result.product_avg_temperature,
    //   result.product_density,
    // );

    let now = Utc::now();
    let base_vars = self.map_calc_result_to_base(&result);
    let ext_vars = self.map_calc_result_to_ext(&meta, &config, entry, &result);

    let base_with_date = BaseVarsWithDate {
      date: now,
      changed_at: now,
      data: base_vars,
    };

    let ext_with_date = ExtVarsWithDate {
      date: now,
      changed_at: now,
      data: ext_vars,
    };

    // Запись base_vars (как и раньше, только данные без даты)
    let base_yaml = serde_saphyr::to_string(&base_with_date.data)?;
    fs::write(&base_vars_path, base_yaml)?;

    // Запись ext_vars
    let ext_yaml = serde_saphyr::to_string(&ext_with_date.data)?;
    fs::write(&ext_vars_path, ext_yaml)?;

    // Обновляем previous_result для этого танка
    state.previous_results.insert(*tank_id, result);

    Ok(())
  }

  /// Собираем структуру Calculation из meta/config/entry и предыдущего результата
  fn build_calculation(
    &self,
    meta: &Meta,
    config: &TankConfig,
    entry: &TimeSeriesEntry,
    prev_result: Option<CalculationResult>,
  ) -> Calculation {
    let c = &meta.constants;

    let calc_constants = Constants {
      calculation_method: CalculationMethod::try_from(c.calculation_method as u8)
        .unwrap_or_default(),
      product_type: ProductType::try_from(c.product_type as u8).unwrap_or_default(),
      pontoon_weight: c.pontoon_weight,

      tank_wall_alpha: c.tank_wall_alpha,
      distance_abs_error_limit: c.distance_abs_error_limit,
      p1_measuring_range_max: c.p1_measuring_range_max,
      pressure1_proc_error_limit: c.pressure1_proc_error_limit,
      pressure3_max_limit: c.pressure3_max_limit,
      pressure3_proc_error_limit: c.pressure3_proc_error_limit,
      max_level_abs_error: c.max_level_abs_error,
      water_level_abs_error_limit: c.water_level_abs_error_limit,
      grad_error_limit: c.grad_error_limit,
      temp_abs_error_limit: c.temp_abs_error_limit,
      calc_error_limit: c.calc_error_limit,
      structure_base_height: c.structure_base_height,

      tank_product_density: c.tank_product_density,
      product_density_15:c.product_density_15,
      g: c.g,
      air_density: c.air_density,
      product_initial_boil_temp: c.product_initial_boil_temp,
      p1_p3_distance: c.p1_p3_distance,
      h_calibration_coefficient: c.h_calibration_coefficient,
      h_critical_level: c.h_critical_level,
      hysteresis_temperature_sensor_level: c.hysteresis_temperature_sensor_level,
      hysteresis_product_level_for_method_type: c.hysteresis_product_level_for_method_type,
      h_max_level: c.h_max_level,
      reference_point: c.reference_point,
      pressure_sensor_to_reference_point: c.pressure_sensor_to_reference_point,
      density_abs_error_limit: c.density_abs_error_limit,
      water_mass_fraction_abs_error_limit: c.water_mass_fraction_abs_error_limit,
      mechanical_impurities_abs_error_limit: c.mechanical_impurities_abs_error_limit,
      chlorides_mass_fraction_abs_error_limit: c.chlorides_mass_fraction_abs_error_limit,
      water_mass_pct: c.water_mass_pct,
      mech_impurities_mass_pct: c.mech_impurities_mass_pct,
      chloride_salts_mass_pct: c.chloride_salts_mass_pct,
    };

    let graduation_table: Vec<GradTableItem> = meta
      .grad_table
      .iter()
      .filter_map(|row| {
        if row.len() >= 2 {
          Some(GradTableItem {
            level: row[0],
            volume: row[1],
            epsilon: if row.len() >= 3 { row[2] } else { 0.0 },
          })
        } else {
          None
        }
      })
      .collect();

    let temp_levels: HashMap<_, f64> = config
      .levels_of_point_sensors
      .points
      .iter()
      .map(|p| (p.id.to_uppercase(), p.value as f64))
      .collect();

    let level_temps_vec = vec![
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

    let level_temps = level_temps_vec
      .into_iter()
      .filter_map(|(name, temp)| {
        let level = temp_levels.get(name)?;
        Some(TemperatureSensor {
          temperature: temp,
          calibration_coefficient: 0.0,
          t_level: *level,
        })
      })
      .collect::<Vec<_>>();

    let calc_variables = Variables {
      timestamp: entry.ts,
      pressure1: entry.p1,
      pressure3: entry.p3,
      product_level_measured: entry.h_measured,
      water_level: entry.h_v,
      level_temps: Some(level_temps),
      product_density_from_sensor: Some(entry.density),
    };

    Calculation {
      graduation_table,
      constants: calc_constants,
      variables: calc_variables,
      previous_result: prev_result,
      // Пока без отдельного результата "10 секунд назад"
      results_offset_10: None,
      // None => будет использован DEFAULT_EVAPORATION_CONSTANTS из ядра
      beta_coefficients: None,
    }
  }

  /// Маппинг CalculationResult -> BaseVars
  fn map_calc_result_to_base(&self, result: &CalculationResult) -> BaseVars {
    BaseVars {
      // Масса – брутто из ядра (тонны)
      weight: result.gross_product_mass,
      // Рабочий объем – V при рабочих условиях
      work_calc_vol: result.product_volume,
      // Средняя температура продукта
      product_avg_temp: result.product_avg_temperature,
      // Плотность при условиях измерения
      product_dens: result.product_density,
    }
  }

  /// Маппинг CalculationResult -> ExtVars
  fn map_calc_result_to_ext(
    &self,
    meta: &Meta,
    config: &TankConfig,
    entry: &TimeSeriesEntry,
    result: &CalculationResult,
  ) -> ExtVars {
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
      product_volume: Some(result.product_volume),
      product_level: Some(result.product_level),
      water_level: Some(entry.h_v),
      product_temperature: Some(result.product_avg_temperature),
      vapour_temperature: Some(result.vapor_avg_temperature),
      product_density: Some(result.product_density),
      product_at_15_density: Some(result.product_density_15),
      hydrostatic_pressure: Some(entry.p1),
      vapour_pressure: Some(entry.p3),
      reserve_capacity_up_max: Some(meta.constants.h_max_level),
      reserve_product_up_min: Some(meta.constants.h_critical_level),
      product_movement_consumption: Some(0.0),
      product_movement_level_measurement_speed: Some(result.velocity_product_level),
      volume_product_calc_below_water: Some(result.water_volume),
      volume_raw_water: Some(result.water_volume),
      temperatures,
    }
  }
}
