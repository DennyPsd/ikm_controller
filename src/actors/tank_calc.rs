use chrono::{DateTime, Local, Utc};
use ikm_calc::calculation::core::{
  Calculation, CalculationMethod, CalculationResult, Constants, ProductType, TemperatureSensor,
  Variables,
};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use crate::types::{
  tank_configuration::TankConfig,
  tanks::{BaseVars, ExtVars, Tank},
};

use crate::types::type_traits::CalculationResultExt;
use ikm_calc::calculation::types::GradTableItem;
use taxon_core::components::data::DataModel;
use taxon_core::components::device::{FacilityEvent, FacilityEventRule};

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
}

/// Структура для парсинга данных из emulation.csv
#[allow(dead_code)]
#[derive(Debug)]
pub struct EmulationEntry {
  pub time_stap: i64,
  pub hydrostatic_pressure: f64,
  pub vapour_pressure: f64,
  pub h_measured: f64,
  pub water_level: f64,
  pub product_density: f64,
  pub temperatures: [f64; 10],
}

impl EmulationEntry {
  fn to_time_series_entry(&self) -> TimeSeriesEntry {
    TimeSeriesEntry {
      ts: self.time_stap,
      // hydrostatic_pressure - это p1 (давление P1)
      p1: self.hydrostatic_pressure,
      p3: self.vapour_pressure,
      h_measured: self.h_measured,
      h_v: self.water_level,
      density: self.product_density,
      t0: self.temperatures[0],
      t1: self.temperatures[1],
      t2: self.temperatures[2],
      t3: self.temperatures[3],
      t4: self.temperatures[4],
      t5: self.temperatures[5],
      t6: self.temperatures[6],
      t7: self.temperatures[7],
      t8: self.temperatures[8],
      t9: self.temperatures[9],
    }
  }
}

/// Парсит CSV строку в EmulationEntry
/// Формат CSV: time_stap;hydrostatic_pressure;vapour_pressure;h_measured;water_level;product_density;t0;t1;...;t9
/// Разделитель - точка с запятой (;), числа могут использовать запятую или точку как десятичный разделитель
fn parse_csv_line(line: &str) -> Option<EmulationEntry> {
  // Сначала заменяем запятую на точку (для поддержки европейского формата чисел)
  let normalized_line = line.replace(',', ".");
  let parts: Vec<&str> = normalized_line.split(';').map(|s| s.trim()).collect();

  if parts.len() < 16 {
    return None;
  }

  let time_stap = parts[0].parse::<i64>().ok()?;
  let hydrostatic_pressure = parts[1].parse::<f64>().ok()?;
  let _vapour_pressure = parts[2].parse::<f64>().ok()?;
  let h_measured = parts[3].parse::<f64>().ok()?;
  let water_level = parts[4].parse::<f64>().ok()?;
  let product_density = parts[5].parse::<f64>().ok()?;

  let mut temperatures = [0.0; 10];
  for i in 0..10 {
    if let Some(temp) = parts.get(6 + i) {
      temperatures[i] = temp.parse::<f64>().unwrap_or(0.0);
    }
  }

  Some(EmulationEntry {
    time_stap,
    hydrostatic_pressure,
    vapour_pressure: _vapour_pressure,
    h_measured,
    water_level,
    product_density,
    temperatures,
  })
}

/// Структура для внутреннего использования ( old format )
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
  /// Состояние по правилам событий: (tank_id, rule_id) -> EventState
  pub event_states: HashMap<(Uuid, Uuid), EventState>,
  pub events: Option<Vec<FacilityEvent>>,
}

/// Внутреннее состояние нарушения правила для конкретного танка/правила
#[derive(Debug)]
pub struct EventState {
  /// ts (unix time), когда началось непрерывное нарушение условия правила
  exceed_started_ts: Option<i64>,
  /// Уже сгенерировали событие, чтобы не спамить
  event_active: bool,
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
      event_states: HashMap::new(),
      events: None,
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

        let mut tanks = Tank::load_list().await;
        let event_rules = FacilityEventRule::load_list().await;
        let events = FacilityEvent::load_list().await;
        state.events = Some(events);

        for tank in tanks.iter_mut() {
          let tank_id = tank.id;
          if let Err(e) = self.process_tank(tank, state, &event_rules).await {
            error!("Ошибка ID tank {}: {}", tank_id, e);
          }
        }
        if state.events.is_some()
          && !state.events.as_ref().unwrap().is_empty()
          && let Err(e) = FacilityEvent::save_many(state.events.take().unwrap()).await
        {
          error!("Ошибка обновления событий: {}", e);
        }

        let _ = myself.send_after(Duration::from_secs(5), || TankCalcMsg::Tick);
      }
    }

    Ok(())
  }
}

impl TankCalcActor {
  async fn process_tank(
    &self,
    tank: &mut Tank,
    state: &mut TankCalcState,
    event_rules: &Vec<FacilityEventRule>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let meta_path = format!("assets/db/calc/{}/meta.json", tank.id);
    let emulation_path = format!("assets/db/tanks/{}/emulation.csv", tank.id);
    let config_vars_path = format!("assets/db/tanks/{}/config.yaml", tank.id);
    let base_vars_path = format!("assets/db/tanks/{}/base_vars.yaml", tank.id);
    let ext_vars_path = format!("assets/db/tanks/{}/ext_vars.yaml", tank.id);

    let grad_table_path = format!("assets/db/tanks/{}/grad_table.json", tank.id);

    // Читаем meta.json
    let meta_content = fs::read_to_string(&meta_path)?;
    let meta: Meta =
      serde_json::from_str(&meta_content).map_err(|err| format!("Cant parse meta: {err:?}"))?;

    // Читаем config.yaml
    let config_content = fs::read_to_string(&config_vars_path)?;
    let config: TankConfig = serde_saphyr::from_str(&config_content)
      .map_err(|err| format!("Cant parse config: {err:?}"))?;

    // Проверяем режим эмуляции
    // Если emulation == false, то данные пишутся из ModbusWorker, tank_calc не нужен
    // Если emulation == true или не указан (по умолчанию true для совместимости), то используем tank_calc
    let is_emulation = config
      .modbus
      .as_ref()
      .and_then(|m| m.emulation)
      .unwrap_or(true);

    if !is_emulation {
      // Режим реальных датчиков - tank_calc не запускаем, данные пишутся из ModbusWorker
      info!("TankCalc: танк {} в режиме реальных датчиков", tank.id);
      return Ok(());
    }

    //info!("TankCalc: обрабатываем танк {} (режим эмуляции)", tank.id);

    let grad_rows = match fs::read_to_string(&grad_table_path) {
      Ok(content) => match serde_json::from_str::<Vec<Vec<f64>>>(&content) {
        Ok(rows) => {
          if rows.is_empty() {
            error!(
              "TankCalc: grad_table.json для {} прочитан, но в нём 0 строк — пропускаем расчёт",
              tank.id
            );
            return Ok(());
          }
          rows
        }
        Err(err) => {
          error!(
            "TankCalc: не удалось распарсить {} как Vec<Vec<f64>>: {err:?} — пропускаем расчёт для {}",
            grad_table_path, tank.id
          );
          return Ok(());
        }
      },
      Err(err) if err.kind() == ErrorKind::NotFound => {
        error!(
          "TankCalc: grad_table.json для {} не найден ({:?}) — без него расчёт невозможен, пропускаем",
          tank.id, err
        );
        return Ok(());
      }
      Err(err) => {
        error!(
          "TankCalc: не удалось прочитать {}: {err:?} — пропускаем расчёт для {}",
          grad_table_path, tank.id
        );
        return Ok(());
      }
    };
    // Читаем emulation.json из папки tanks/{tank_id}/
    let emulation_content = match fs::read_to_string(&emulation_path) {
      Ok(content) => content,
      Err(err) if err.kind() == ErrorKind::NotFound => {
        // Файла emulation.csv нет - при включенной эмуляции ничего не делаем
        // info!(
        //   "TankCalc: emulation.csv для {} не найден - пропускаем",
        //   tank.id
        // );
        return Ok(());
      }
      Err(err) => {
        error!(
          "TankCalc: не удалось прочитать emulation.csv: {err:?} — пропускаем расчёт для {}",
          tank.id
        );
        return Ok(());
      }
    };

    // Парсим CSV файл emulation.csv
    let emulation_data: Vec<EmulationEntry> = emulation_content
      .lines()
      .skip(1) // Пропускаем заголовок
      .filter_map(|line| {
        let line = line.trim();
        if line.is_empty() {
          return None;
        }
        parse_csv_line(line)
      })
      .collect();

    if emulation_data.is_empty() {
      error!(
        "TankCalc: не удалось распарсить emulation.csv для {} — пропускаем",
        tank.id
      );
      return Ok(());
    }

    if emulation_data.is_empty() {
      return Ok(());
    }

    // Берем индекс из emulation_data, с учетом того, что они могут повторяться
    let current_index = state.current_indices.entry(tank.id).or_insert(0);
    let next_index = (*current_index + 1) % emulation_data.len();
    let entry = &emulation_data[next_index];
    *current_index = next_index;

    // Конвертируем EmulationEntry в TimeSeriesEntry для совместимости с расчетом
    let ts_entry = entry.to_time_series_entry();

    let prev_result = state.previous_results.get(&tank.id).cloned();

    let calc = self.build_calculation(&meta, &config, &ts_entry, prev_result, &grad_rows);
    // println!("calc : {:?}",calc);
    // Запуск расчета ядра
    let result = match calc.calculate() {
      Ok(res) => res,
      Err(e) => {
        error!("Ошибка расчета для {}: {:?}", tank.id, e);
        return Ok(());
      }
    };
    // println!("CCCCAAAALLLLCCCC!!!!!  {:?} ", calc,);
    // println!("RESSUUUUULLLLLLTTTTT {:?}", result);
    let now = Utc::now();

    // Маппинг через экстеншен
    let base_vars = result.to_base_vars();
    let ext_vars = result.to_ext_vars(&meta, &config, &ts_entry);

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

    // Запись base_vars
    let base_yaml = serde_saphyr::to_string(&base_with_date.data)?;
    fs::write(&base_vars_path, base_yaml)?;

    // Запись ext_vars
    let ext_yaml = serde_saphyr::to_string(&ext_with_date.data)?;
    fs::write(&ext_vars_path, ext_yaml)?;

    // Проверяем правила событий и при необходимости генерируем FacilityEvent
    self.check_event_rules(tank, event_rules, &result, entry.time_stap, state);

    // Обновляем previous_result для этого танка
    state.previous_results.insert(tank.id, result);

    Ok(())
  }

  /// Собираем структуру Calculation из meta/config/entry и предыдущего результата
  fn build_calculation(
    &self,
    meta: &Meta,
    config: &TankConfig,
    entry: &TimeSeriesEntry,
    prev_result: Option<CalculationResult>,
    grad_rows: &[Vec<f64>],
  ) -> Calculation {
    // Constants уже десериализованы как есть
    let mut calc_constants: Constants = meta.constants;
    calc_constants.product_type = if config
      .basic_data
      .product
      .clone()
      .unwrap_or("default".into())
      .contains("OilProduct")
    {
      ProductType::OilProduct
    } else {
      ProductType::Oil
    };
    calc_constants.calculation_method = match config
      .mass_calculation_method
      .method
      .as_ref()
      .map(|v| &v[..])
    {
      Some("1") => CalculationMethod::One,
      Some("2") => CalculationMethod::Two,
      Some("3") => CalculationMethod::Three,
      Some("4") => CalculationMethod::Four,
      _ => CalculationMethod::One,
    };

    // calc_constants.h_critical_level = config
    //   .mass_calculation_method
    //   .switching_level
    //   .unwrap_or(0.0);
    // calc_constants.hysteresis_product_level_for_method_type = config
    //   .mass_calculation_method
    //   .switching_level_hysteresis
    //   .unwrap_or(0.0);
    // calc_constants.p1_p3_distance = config.mass_calculation_method.p3_p1.unwrap_or(0.0);
    // calc_constants.pressure_sensor_to_reference_point = config
    //   .mass_calculation_method
    //   .p1_reference_point
    //   .unwrap_or(0.0);
    // calc_constants.reference_point = config
    //   .mass_calculation_method
    //   .reference_point
    //   .unwrap_or(0.0);
    // "pontoon_weight": 2877,
    // "tank_wall_alpha": 0.0000125,

    // "distance_abs_error_limit": 0.001,
    // "p1_measuring_range_max": 110000,
    // "pressure1_proc_error_limit": 0.04,
    // "pressure3_max_limit": 10000,
    // "pressure3_proc_error_limit": 0.04,
    // "max_level_abs_error": 1,
    // "water_level_abs_error_limit": 1,
    // "grad_error_limit": 0.15,
    // "temp_abs_error_limit": 0.5,
    // "calc_error_limit": 0.05,
    // "structure_base_height": 0,
    // "tank_product_density": 738,

    // "p1_p3_distance": 12758,
    // "h_calibration_coefficient": -2,
    // "h_critical_level": 3500,
    // "hysteresis_temperature_sensor_level": 10,
    // "hysteresis_product_level_for_method_type": 2,
    // "h_max_level": 0,
    // "reference_point": 0,
    // "pressure_sensor_to_reference_point": 1242,
    // "density_abs_error_limit": 1,
    // "water_mass_fraction_abs_error_limit": 0,
    // "mechanical_impurities_abs_error_limit": 0,
    // "chlorides_mass_fraction_abs_error_limit": 0,
    // "water_mass_pct": 2,
    // "mech_impurities_mass_pct": 2,
    // "chloride_salts_mass_pct": 3

    // 4. Параметры метода расчёта — по максимуму из конфигурации
    if let Some(sw) = config.mass_calculation_method.switching_level {
      calc_constants.h_critical_level = sw;
    }

    if let Some(hyst) = config.mass_calculation_method.switching_level_hysteresis {
      calc_constants.hysteresis_product_level_for_method_type = hyst;
    }

    if let Some(d) = config.mass_calculation_method.p3_p1 {
      calc_constants.p1_p3_distance = d;
    }

    if let Some(p1_ref) = config.mass_calculation_method.p1_reference_point {
      calc_constants.pressure_sensor_to_reference_point = p1_ref;
    }

    if let Some(rp) = config.mass_calculation_method.reference_point {
      calc_constants.reference_point = rp;
    }
    // 5. Конструкция — из construction
    if let Some(alpha) = config.construction.linear_expansion {
      // коэффициент линейного расширения стенки резервуара
      calc_constants.tank_wall_alpha = alpha as f64;
    }

    if let Some(pontoon) = config.construction.mass_floating_coating {
      // масса понтона
      calc_constants.pontoon_weight = Some(pontoon as f64);
    }

    // 6. Базовые данные: из config.basic_data
    if let Some(h) = config.basic_data.basic_height {
      // базовая высота резервуара
      calc_constants.structure_base_height = h as f64;
    }

    if let Some(rho) = config.basic_data.density_stored_liquid_according {
      // плотность продукта в резервуаре
      calc_constants.tank_product_density = Some(rho as f64);
    }

    if let Some(h_max) = config.basic_data.maximum_allowable_product_level {
      // максимальный допустимый уровень продукта
      calc_constants.h_max_level = h_max as f64;
    }

    // 7. Показатели точности измерений: из config.measurement_accuracy_indicators
    let mai = &config.measurement_accuracy_indicators;

    if let Some(v) = mai.limit_permissible_absolute_error {
      // допустимая абсолютная погрешность расстояния
      calc_constants.distance_abs_error_limit = v as f64;
    }

    if let Some(v) = mai.hydrostatic_device_limit {
      // предел гидростатического устройства — диапазон P1
      calc_constants.p1_measuring_range_max = v as f64;
    }

    if let Some(v) = mai.pressure_device_limit {
      // предел устройства давления — максимум P3
      calc_constants.pressure3_max_limit = v as f64;
    }

    if let Some(v) = mai.limit_permissible_absolute_measurement_reservoir_level {
      // погрешность измерения уровня резервуара
      calc_constants.max_level_abs_error = v as f64;
    }

    if let Some(v) = mai.limit_permissible_absolute_measurement_level_raw_water {
      // погрешность измерения уровня сырой воды
      calc_constants.water_level_abs_error_limit = v as f64;
    }

    if let Some(v) = mai.limit_permissible_absolute_measurement_temp_products_and_vapours {
      // погрешность измерения температуры продуктов и паров
      calc_constants.temp_abs_error_limit = v as f64;
    }

    if let Some(v) = mai.limit_permissible_absolute_measurement_oil_densities {
      // погрешность измерения плотности нефти
      calc_constants.density_abs_error_limit = v as f64;
    }

    // 8. Блок калибровки — коэффициент уровня
    if let Some(k) = config.calibration_block.level_coefficient {
      calc_constants.h_calibration_coefficient = k;
    }

    // 9. Гистерезис точечных датчиков уровня
    if let Some(hyst) = config.levels_of_point_sensors.hysteresis {
      // логично положить в гистерезис по датчикам/температурным уровням
      calc_constants.hysteresis_temperature_sensor_level = hyst;
    }

    let graduation_table: Vec<GradTableItem> = grad_rows
      .iter()
      .filter_map(|row| {
        if row.len() >= 2 {
          Some(GradTableItem {
            level: row[0],
            volume: row[1],
            // epsilon: if row.len() >= 3 { row[2] } else { 0.0 },
          })
        } else {
          None
        }
      })
      .collect();

    let level_mm_by_id: HashMap<String, f64> = config
      .levels_of_point_sensors
      .points
      .iter()
      .map(|p| (p.id.to_uppercase(), p.value as f64))
      .collect();

    let kti_by_id: HashMap<String, f64> = config
      .calibration_block
      .level_point_sensors
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
        let key = name.to_uppercase();
        let level = level_mm_by_id.get(&key)?;
        let kti = kti_by_id.get(&key).cloned().unwrap_or(0.0);

        Some(TemperatureSensor {
          temperature: temp,
          calibration_coefficient: kti,
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
      results_offset_10: None,
      beta_coefficients: None,
    }
  }

  /// Проверка всех FacilityEventRule из eventrules.yaml и генерация FacilityEvent в events.yaml
  fn check_event_rules(
    &self,
    tank: &Tank,
    event_rules: &Vec<FacilityEventRule>,
    result: &CalculationResult,
    entry_ts: i64,
    state: &mut TankCalcState,
  ) {
    if event_rules.is_empty() {
      return;
    }

    for rule in event_rules {
      let Some(target) = rule.target() else {
        continue;
      };

      match &target.data_ns {
        Some(data_ns) if data_ns == "Tank" => {}
        _ => {
          continue;
        }
      }

      match rule {
        FacilityEventRule::LimitsExceeded {
          id,
          var_path,
          min,
          max,
          threshold,
          ..
        } => {
          let value = match self.resolve_var_value(result, var_path) {
            Some(v) => v,
            None => {
              continue;
            }
          };

          let mut exceeded = false;
          if let Some(min_v) = min
            && value < *min_v as f64
          {
            exceeded = true;
          }
          if let Some(max_v) = max
            && value > *max_v as f64
          {
            exceeded = true;
          }

          let key = (tank.id, *id);

          if !exceeded {
            if let Some(ev_state) = state.event_states.get_mut(&key) {
              if ev_state.exceed_started_ts.is_some() || ev_state.event_active {
                // info!(
                //   "TankCalc: значение {:?} для правила {} по танку {} вернулось в норму (value={:.3})",
                //   var_path, id, tank.id, value
                // );
              }
              ev_state.exceed_started_ts = None;
              ev_state.event_active = false;
            }
            continue;
          }

          // Нарушение есть
          let ev_state = state.event_states.entry(key).or_insert(EventState {
            exceed_started_ts: None,
            event_active: false,
          });

          if ev_state.exceed_started_ts.is_none() {
            ev_state.exceed_started_ts = Some(entry_ts);
          }
          #[allow(clippy::manual_unwrap_or)]
          let threshold_secs = (if let Some(dur) = *threshold { dur } else { 0.0 }) as i32;

          if let Some(start_ts) = ev_state.exceed_started_ts {
            let secs = entry_ts.saturating_sub(start_ts) as i32;

            if secs >= threshold_secs && !ev_state.event_active {
              let event = FacilityEvent {
                id: Uuid::now_v7(),
                target: rule.target().cloned(),
                target_title: tank.title().cloned(),
                severity: rule.severity(),
                starts_at: Local::now(),
                ends_at: None,
                rule: rule.as_link(),
                acknowledged: None,
                value,
                comment: None,
              };

              // info!(
              //   "TankCalc: генерируем событие по правилу {} для танка {} (var={:?}, value={:.3}, min={:?}, max={:?}, secs={})",
              //   id, tank.id, var_path, value, min, max, secs
              // );

              state.events.as_mut().unwrap().push(event);
              ev_state.event_active = true;
            }
          }
        }
        FacilityEventRule::HartStatus { .. } => {}
      }
    }
  }

  /// var_path -> конкретное значение из CalculationResult
  fn resolve_var_value(&self, result: &CalculationResult, var_path: &[SmolStr]) -> Option<f64> {
    if var_path.is_empty() {
      return None;
    }

    let name = var_path[0].as_str();

    match name {
      "product_level" => Some(result.product_level),
      "product_volume" => Some(result.product_volume),
      "gross_product_mass" => Some(result.gross_product_mass),
      "product_density" => Some(result.product_density),
      "product_density_15" => Some(result.product_density_15),
      "water_volume" => Some(result.water_volume),
      "capacity_at_current_level" => Some(result.capacity_at_current_level),
      "volume_relative_error_limit" => Some(result.volume_relative_error_limit),
      "gross_mass_relative_error_limit" => Some(result.gross_mass_relative_error_limit),
      "velocity_product_level" => Some(result.velocity_product_level),
      _ => None,
    }
  }
}
