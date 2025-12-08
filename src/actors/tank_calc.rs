use chrono::{DateTime, Utc};
use ikm_calc::calculation::core::{
  Calculation, CalculationResult, Constants, TemperatureSensor, Variables,
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
  tanks::{BaseVars, ExtVars},
};

use crate::types::type_traits::CalculationResultExt;
use ikm_calc::calculation::types::GradTableItem;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct LevelCoefficientPoint {
  pub id: String,
  pub level_mm: f64,
  pub kti: f64,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Meta {
  pub constants: Constants,
  pub level_coefficient_points: Vec<LevelCoefficientPoint>,
  /// grad_table: [level, volume, epsilon?]
  pub grad_table: Vec<Vec<f64>>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct TimeSeriesEntry {
  pub ts: i64,
  pub p1: f64,
  pub p3: f64,
  pub h_measured: f64,
  pub h_v: f64,
  pub density: f64,
  pub t0: f64,
  pub t1: f64,
  pub t2: f64,
  pub t3: f64,
  pub t4: f64,
  pub t5: f64,
  pub t6: f64,
  pub t7: f64,
  pub t8: f64,
  pub t9: f64,
}

#[derive(Serialize, Debug)]
pub struct BaseVarsWithDate {
  date: DateTime<Utc>,
  changed_at: DateTime<Utc>,
  #[serde(flatten)]
  data: BaseVars,
}

#[derive(Serialize, Debug)]
pub struct ExtVarsWithDate {
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

    let now = Utc::now();

    // Маппинг через экстеншен
    let base_vars = result.to_base_vars();
    let ext_vars = result.to_ext_vars(&meta, &config, entry);

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
    // Constants уже десериализованы как есть
    let calc_constants: Constants = meta.constants;

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
}
