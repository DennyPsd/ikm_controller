use chrono::{DateTime, Utc};
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

// Инициализация констант из meta. Полей много, часть пока реально не используется.
// Чтобы clippy не ругался на dead_code, явно разрешаем.
#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct MetaConstants {
  calculation_method: i32,
  product_type: i32,
  pontoon_weight: f64,
  tank_wall_alpha: f64,
  distance_abs_error_limit: f64,
  p1_measuring_range_max: f64,
  pressure1_proc_error_limit: f64,
  pressure3_max_limit: f64,
  pressure3_proc_error_limit: f64,
  max_level_abs_error: f64,
  water_level_abs_error_limit: f64,
  grad_error_limit: f64,
  temp_abs_error_limit: f64,
  calc_error_limit: f64,
  structure_base_height: f64,
  tank_product_density: f64,
  g: f64,
  air_density: f64,
  product_initial_boil_temp: f64,
  p1_p3_distance: f64,
  h_calibration_coefficient: f64,
  h_critical_level: f64,
  hysteresis_temperature_sensor_level: f64,
  hysteresis_product_level_for_method_type: f64,
  h_max_level: f64,
  reference_point: f64,
  pressure_sensor_to_reference_point: f64,
  density_abs_error_limit: f64,
  water_mass_fraction_abs_error_limit: f64,
  mechanical_impurities_abs_error_limit: f64,
  chlorides_mass_fraction_abs_error_limit: f64,
  water_mass_pct: f64,
  mech_impurities_mass_pct: f64,
  chloride_salts_mass_pct: f64,
  product_density_15: f64,
}

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
  constants: MetaConstants,
  level_coefficient_points: Vec<LevelCoefficientPoint>,
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

    // Запуск расчета base и ext
    let now = Utc::now();
    let base_vars = self.calculate_base_vars(&meta, entry, now).data;
    let ext_vars = self.calculate_ext_vars(&meta, &config, entry, now).data;

    // Запись base_vars
    let base_yaml = serde_saphyr::to_string(&base_vars)?;
    fs::write(&base_vars_path, base_yaml)?;

    // Запись ext_vars
    let ext_yaml = serde_saphyr::to_string(&ext_vars)?;
    fs::write(&ext_vars_path, ext_yaml)?;

    // info!("Резервуар обновлен {} ", tank_id);

    Ok(())
  }

  fn calculate_base_vars(
    &self,
    meta: &Meta,
    entry: &TimeSeriesEntry,
    now: DateTime<Utc>,
  ) -> BaseVarsWithDate {
    // РАСЧЕТ BASE_VARS. Можно поменять под наши задачи
    let weight = meta.constants.pontoon_weight * 3.0;
    let work_calc_vol = entry.h_measured * 2.0;
    let product_avg_temp = ((entry.t0
      + entry.t1
      + entry.t2
      + entry.t3
      + entry.t4
      + entry.t5
      + entry.t6
      + entry.t7
      + entry.t8
      + entry.t9)
      / 10.0)
      .round();
    let product_dens = entry.density;

    BaseVarsWithDate {
      date: now,
      changed_at: now,
      data: BaseVars {
        weight,
        work_calc_vol,
        product_avg_temp,
        product_dens,
      },
    }
  }

  fn calculate_ext_vars(
    &self,
    meta: &Meta,
    config: &TankConfig,
    entry: &TimeSeriesEntry,
    now: DateTime<Utc>,
  ) -> ExtVarsWithDate {
    // РАСЧЕТ EXT_VARS
    let product_volume = entry.h_measured * 2.0;
    let product_level = entry.h_measured;
    let water_level = 0.0;
    let product_temperature = (entry.t0 + entry.t9) / 2.0;
    let vapour_temperature = entry.t9;
    let product_density = entry.density;
    let product_at_15_density = meta.constants.product_density_15;
    let hydrostatic_pressure = entry.p1;
    let vapour_pressure = entry.p3;
    let reserve_capacity_up_max = meta.constants.h_max_level;
    let reserve_product_up_min = meta.constants.h_critical_level;
    let product_movement_consumption = 0.0;
    let product_movement_level_measurement_speed = 0.0;
    let volume_product_calc_below_water = 0.0;
    let volume_raw_water = 0.0;
    let temperature_levels: HashMap<_, _> = config
      .levels_of_point_sensors
      .points
      .iter()
      .map(|v| (v.id.to_uppercase(), v.clone()))
      .collect();
    let temperatures = vec![
      Temperature {
        value: entry.t0,
        name: "T0".to_string(),
        level: temperature_levels
          .get("T0")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t1,
        name: "T1".to_string(),
        level: temperature_levels
          .get("T1")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t2,
        name: "T2".to_string(),
        level: temperature_levels
          .get("T2")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t3,
        name: "T3".to_string(),
        level: temperature_levels
          .get("T3")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t4,
        name: "T4".to_string(),
        level: temperature_levels
          .get("T4")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t5,
        name: "T5".to_string(),
        level: temperature_levels
          .get("T5")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t6,
        name: "T6".to_string(),
        level: temperature_levels
          .get("T6")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t7,
        name: "T7".to_string(),
        level: temperature_levels
          .get("T7")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t8,
        name: "T8".to_string(),
        level: temperature_levels
          .get("T8")
          .cloned()
          .unwrap_or_default()
          .value,
      },
      Temperature {
        value: entry.t9,
        name: "T9".to_string(),
        level: temperature_levels
          .get("T9")
          .cloned()
          .unwrap_or_default()
          .value,
      },
    ];

    ExtVarsWithDate {
      date: now,
      changed_at: now,
      data: ExtVars {
        product_volume: Some(product_volume),
        product_level: Some(product_level),
        water_level: Some(water_level),
        product_temperature: Some(product_temperature),
        vapour_temperature: Some(vapour_temperature),
        product_density: Some(product_density),
        product_at_15_density: Some(product_at_15_density),
        hydrostatic_pressure: Some(hydrostatic_pressure),
        vapour_pressure: Some(vapour_pressure),
        reserve_capacity_up_max: Some(reserve_capacity_up_max),
        reserve_product_up_min: Some(reserve_product_up_min),
        product_movement_consumption: Some(product_movement_consumption),
        product_movement_level_measurement_speed: Some(product_movement_level_measurement_speed),
        volume_product_calc_below_water: Some(volume_product_calc_below_water),
        volume_raw_water: Some(volume_raw_water),
        temperatures,
      },
    }
  }
}
