use crate::actors::ipc_handler::get_tank_full;
use crate::actors::tank_calc::Meta;
use crate::msges::kmh_report_create::KMHReportCreateArgs;
use crate::msges::kmh_report_list::{KMHReportListArgs, KMHReportListFields, KMHReportListReply};
use crate::types::kmh::{KMHReportInstance, KMHReportStatus};
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Tank};
use crate::types::type_traits::KMHReportExt;
use chrono::Local;
use ikm_calc::calculation::kmh::{KMHCalculator, KMHReport, TapeClass, TemperatureSensor};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;
use smol_str::SmolStr;
use std::f64;
use std::fs::{self, File};
use std::path::Path;
use taxon_core::actors::ipc::errors::internal_error;
use taxon_core::infrastructure::data::{DataLink, DataModel};
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tracing::{error, info};
use umya_spreadsheet::{reader, writer};
use uuid::Uuid;

pub struct KmhIpcHandlerState {
  pub ipc_router: ActorRef<Option<IPCActorMsg>>,
}

#[derive(Debug)]
pub enum KmhIpcHandlerMsg {
  Ipc(IPCMessageCrate),
}

pub struct KmhIpcHandler;

impl KmhIpcHandler {
  /// «Пустая» болванка отчёта КМХ.
  /// Используем:
  ///  - при выдаче списка с fields = Minimal,
  ///  - как базу при создании отчёта (можно переиспользовать и в create).
  fn empty_kmh_report() -> KMHReport {
    KMHReport {
      // Окружение
      air_temperature_outside: 0.0,
      air_pressure_outside: 0.0,
      wind_speed: 0.0,
      gas_layer_height_measured_points: Vec::new(),
      measured_height: 0.0,
      nominal_height: 0.0,
      base_measured_height: f64::NAN,
      delta_height: 0.0,

      // Плотности
      density_verified: 0.0,
      density_measured: 0.0,
      density_measured_controlled: (None, None, None),

      // Объёмы / массы
      product_volume_measured: 0.0,
      volume_coarse: 0.0,
      air_temp_verify: 0.0,
      pontoon_mass: 0.0,
      product_mass_measured: 0.0,

      // Температура паров
      vapor_temp: 0.0,

      // Рулетка / допуски
      tape_class: TapeClass::default(),
      delta_v_max: 0.0,
      delta_m_max: 0.0,
      ruler_alpha_coefficient: 0.0,
      wall_alpha_coefficient: 0.0,
      pressure_coefficient: 0.0,

      // Каналы
      level_channels: None,
      temperature_channels: Vec::new(),
      mass_channels: None,
      volume_channels: None,
      density_channels: None,
    }
  }

  /// ВСЕГДА забиваем temperature_channels из ext_vars.temperatures
  /// (temperature_controlled пока ставим равным измеренному, чтобы таблица была заполнена).
  fn fill_temperature_channels_from_ext(report: &mut KMHReport, ext: &ExtVars) {
    // ext.temperatures ожидается как Vec<{ value, level, name }>
    // Если у тебя temperatures = Option<Vec<_>>, то поменяй на:
    // let temps = ext.temperatures.as_deref().unwrap_or(&[]);
    let temps = &ext.temperatures;
    info!(
      "fill_temperature_channels_from_ext: ext.temperatures.len={} (before fill report.temperature_channels.len={})",
      temps.len(),
      report.temperature_channels.len()
    );

    if temps.is_empty() {
      report.temperature_channels.clear();
      info!(
        "fill_temperature_channels_from_ext: temps empty -> cleared report.temperature_channels"
      );
      return;
    }

    let mut channels: Vec<TemperatureSensor> = Vec::with_capacity(temps.len());
    for t in temps.iter() {
      let mut ch = TemperatureSensor::default();
      ch.level = t.level as f64;
      ch.temperature = t.value as f64;
      ch.temperature_controlled = t.value as f64;
      channels.push(ch);
    }
    report.temperature_channels = channels;

    info!(
      "fill_temperature_channels_from_ext: AFTER fill report.temperature_channels.len={}",
      report.temperature_channels.len()
    );
    if let Some(first) = report.temperature_channels.get(0) {
      info!(
        "fill_temperature_channels_from_ext: first sensor = {:?}",
        first
      );
    }
  }

  /// Определяем статус КМХ-отчёта по результатам расчёта.
  /// Логика:
  ///  - если вообще нет ни одного канала с результатами — FullfilRequired;
  ///  - если есть каналы и ВСЕ имеющиеся passed=true — Positive;
  ///  - иначе — Negative.
  fn status_from_report(report: &KMHReport) -> KMHReportStatus {
    let level_ok = report
      .level_channels
      .as_ref()
      .map(|c| c.passed)
      .unwrap_or(false);

    let mass_ok = report
      .mass_channels
      .as_ref()
      .map(|c| c.passed)
      .unwrap_or(false);

    let volume_ok = report
      .volume_channels
      .as_ref()
      .map(|c| c.passed)
      .unwrap_or(false);

    let density_ok = report
      .density_channels
      .as_ref()
      .map(|c| c.passed)
      .unwrap_or(false);

    let any_channel_present = report.level_channels.is_some()
      || report.mass_channels.is_some()
      || report.volume_channels.is_some()
      || report.density_channels.is_some();

    if !any_channel_present {
      KMHReportStatus::FullfilRequired
    } else if level_ok && mass_ok && volume_ok && density_ok {
      KMHReportStatus::Positive
    } else {
      KMHReportStatus::Negative
    }
  }
}

#[ractor::async_trait]
impl Actor for KmhIpcHandler {
  type Msg = KmhIpcHandlerMsg;
  type State = KmhIpcHandlerState;
  type Arguments = KmhIpcHandlerState;

  async fn pre_start(
    &self,
    _myself: ActorRef<Self::Msg>,
    args: Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    info!("KmhIpcHandler запущен");
    Ok(args)
  }

  async fn handle(
    &self,
    _myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut Self::State,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      KmhIpcHandlerMsg::Ipc(ipc_msg) => {
        if let Some(action) = ipc_msg.as_action() {
          // ===================== KMH_REPORT_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("kmh_report_list")
            && action.args.is_some()
          {
            let args =
              match serde_json::from_value::<KMHReportListArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_list: {}", err));

                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_list: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            let dir_path = "assets/db/kmh_reports";
            let mut reports: Vec<KMHReportInstance> = Vec::new();

            if args.ids.is_empty() {
              match fs::read_dir(dir_path) {
                Ok(entries) => {
                  for entry in entries {
                    let entry = match entry {
                      Ok(e) => e,
                      Err(err) => {
                        error!("kmh_report_list: read_dir entry error: {err:?}");
                        continue;
                      }
                    };

                    let path = entry.path();
                    if !path.is_file() {
                      continue;
                    }
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                      continue;
                    }

                    let path_str = path.to_string_lossy().to_string();

                    match File::open(&path) {
                      Ok(f) => match serde_json::from_reader::<File, KMHReportInstance>(f) {
                        Ok(instance) => {
                          reports.push(instance);
                        }
                        Err(err) => {
                          error!(
                            "kmh_report_list: не удалось распарсить {}: {err:?}",
                            path_str
                          );
                        }
                      },
                      Err(err) => {
                        error!("kmh_report_list: не удалось открыть {}: {err:?}", path_str);
                      }
                    }
                  }
                }
                Err(err) => {
                  if err.kind() != std::io::ErrorKind::NotFound {
                    let err = internal_error(action.name.clone(), None)
                      .with_message(format!("kmh_report_list: read_dir {dir_path}: {err:?}"));

                    if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                      let _ = state.ipc_router.send_message(Some(msg));
                    } else {
                      error!("kmh_report_list: to_replay_msg вернул None при ошибке read_dir");
                    }
                    return Ok(());
                  }
                }
              }
            } else {
              for id in &args.ids {
                let file_path = format!("{}/{}.json", dir_path, id);

                match File::open(&file_path) {
                  Ok(f) => match serde_json::from_reader::<File, KMHReportInstance>(f) {
                    Ok(instance) => {
                      reports.push(instance);
                    }
                    Err(err) => {
                      error!(
                        "kmh_report_list: не удалось распарсить {}: {err:?}",
                        file_path
                      );
                    }
                  },
                  Err(_err) => {}
                }
              }
            }

            let reports = match args.fields {
              KMHReportListFields::Minimal => reports
                .into_iter()
                .map(|mut r| {
                  r.data = KmhIpcHandler::empty_kmh_report();
                  r.tank = r.tank.into_link_sync();
                  r
                })
                .collect::<Vec<_>>(),
              KMHReportListFields::All => reports
                .into_iter()
                .map(|mut r| {
                  let tl = r.tank.into_link_sync();
                  r.tank = get_tank_full(*tl.id()).map(DataLink::Data).unwrap_or(tl);
                  r
                })
                .collect::<Vec<_>>(),
              KMHReportListFields::Exact(_fields) => reports,
            };

            let reply = KMHReportListReply { data: reports };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(reply)), None) {
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_report_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== KMH_REPORT_CREATE =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("kmh_report_create")
            && action.args.is_some()
          {
            let args =
              match serde_json::from_value::<KMHReportCreateArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_create: {}", err));

                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_create: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            let tanks: Vec<Tank> = Tank::load_list().await;

            let tank = match tanks.into_iter().find(|t| t.id == args.device_id) {
              Some(t) => t,
              None => {
                let err = internal_error(action.name.clone(), None).with_message(format!(
                  "kmh_report_create: tank with id={} not found",
                  args.device_id
                ));

                if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("kmh_report_create: to_replay_msg вернул None (tank not found)");
                }
                return Ok(());
              }
            };

            let config_path = format!("assets/db/tanks/{}/config.yaml", args.device_id);
            let base_vars_path = format!("assets/db/tanks/{}/base_vars.yaml", args.device_id);
            let ext_vars_path = format!("assets/db/tanks/{}/ext_vars.yaml", args.device_id);
            let meta_path = format!("assets/db/calc/{}/meta.json", args.device_id);

            let config: Option<TankConfig> = match File::open(&config_path) {
              Ok(f) => match serde_saphyr::from_reader::<File, TankConfig>(f) {
                Ok(v) => Some(v),
                Err(err) => {
                  error!(
                    "kmh_report_create: не удалось распарсить config {}: {err:?}",
                    config_path
                  );
                  None
                }
              },
              Err(_err) => None,
            };

            let base_vars: Option<BaseVars> = match File::open(&base_vars_path) {
              Ok(f) => match serde_saphyr::from_reader::<File, BaseVars>(f) {
                Ok(v) => Some(v),
                Err(err) => {
                  error!(
                    "kmh_report_create: не удалось распарсить base_vars {}: {err:?}",
                    base_vars_path
                  );
                  None
                }
              },
              Err(_err) => None,
            };

            let ext_vars: Option<ExtVars> = match File::open(&ext_vars_path) {
              Ok(f) => match serde_saphyr::from_reader::<File, ExtVars>(f) {
                Ok(v) => {
                  info!(
                    "kmh_report_create: ext_vars loaded from {} | temperatures.len={}",
                    ext_vars_path,
                    v.temperatures.len()
                  );
                  for (i, t) in v.temperatures.iter().enumerate().take(5) {
                    info!(
                      "kmh_report_create: ext_vars.temp[{}] level={} value={} name={}",
                      i, t.level, t.value, t.name
                    );
                  }
                  Some(v)
                }
                Err(err) => {
                  error!(
                    "kmh_report_create: FAILED to parse ext_vars {}: {err:?}",
                    ext_vars_path
                  );
                  None
                }
              },
              Err(err) => {
                error!(
                  "kmh_report_create: FAILED to open ext_vars {}: {err:?}",
                  ext_vars_path
                );
                None
              }
            };

            let meta: Option<Meta> = match File::open(&meta_path) {
              Ok(f) => match serde_json::from_reader::<File, Meta>(f) {
                Ok(v) => Some(v),
                Err(err) => {
                  error!(
                    "kmh_report_create: не удалось распарсить meta {}: {err:?}",
                    meta_path
                  );
                  None
                }
              },
              Err(_err) => None,
            };

            let _parse_opt_f64 = |s: &Option<String>| {
              s.as_ref()
                .and_then(|v| v.replace(',', ".").parse::<f64>().ok())
            };

            let mut kmh_report = KMHReport {
              air_temperature_outside: 0.0,
              air_pressure_outside: 0.0,
              wind_speed: 0.0,
              gas_layer_height_measured_points: Vec::new(),
              measured_height: 0.0,
              nominal_height: 0.0,
              base_measured_height: f64::NAN,
              delta_height: 0.0,

              density_verified: 0.0,
              density_measured: 0.0,
              density_measured_controlled: (None, None, None),

              product_volume_measured: 0.0,
              volume_coarse: 0.0,
              air_temp_verify: 0.0,
              pontoon_mass: 0.0,
              product_mass_measured: 0.0,

              vapor_temp: 0.0,

              tape_class: TapeClass::default(),
              delta_v_max: 0.0,
              delta_m_max: 0.0,
              ruler_alpha_coefficient: 0.0,
              wall_alpha_coefficient: 0.0,
              pressure_coefficient: 0.0,

              level_channels: None,
              temperature_channels: Vec::new(),
              mass_channels: None,
              volume_channels: None,
              density_channels: None,
            };

            if let Some(m) = &meta {
              kmh_report.apply_constants(&m.constants);
            }

            if let Some(cfg) = &config {
              if let Some(h) = &cfg.basic_data.basic_height {
                kmh_report.nominal_height = *h as f64;
              }

              if let Some(a) = &cfg.construction.linear_expansion {
                kmh_report.wall_alpha_coefficient = *a as f64;
              }

              if let Some(m) = &cfg.construction.mass_floating_coating {
                kmh_report.pontoon_mass = *m as f64;
              }

              if let Some(dh) = &cfg
                .measurement_accuracy_indicators
                .limit_permissible_absolute_measurement_reservoir_level
              {
                kmh_report.delta_height = *dh as f64;
              }

              if let Some(dh) = &cfg.basic_data.air_temp_verify {
                kmh_report.air_temp_verify = *dh as f64;
              }
            }

            // apply_base_ext (если есть оба)
            if let (Some(base), Some(ext)) = (&base_vars, &ext_vars) {
              kmh_report.apply_base_ext(base, ext);
            }
            info!(
              "kmh_report_create: AFTER apply_base_ext temperature_channels.len={}",
              kmh_report.temperature_channels.len()
            );

            // ВАЖНО: температура всегда берётся из ext_vars.temperatures и кладётся в temperature_channels
            if let Some(ext) = &ext_vars {
              KmhIpcHandler::fill_temperature_channels_from_ext(&mut kmh_report, ext);
            }
            info!(
              "kmh_report_create: AFTER fill_temperature_channels_from_ext temperature_channels.len={}",
              kmh_report.temperature_channels.len()
            );

            let tank_link = tank.as_link();

            let now = Local::now();
            let report_id = Uuid::now_v7();

            let kmh_instance = KMHReportInstance {
              id: report_id,
              title: Some(format!("КМХ отчёт для цистерны {}", args.device_id).into()),
              description: None,
              created_by: SmolStr::new("system"),
              updated_by: None,
              created_at: now,
              updated_at: None,
              tank: tank_link,
              software_name: Some(SmolStr::new("ikm")),
              software_version: Some(SmolStr::new("0.1.1")),
              status: KMHReportStatus::FullfilRequired,
              data: kmh_report,
            };

            let dir_path = "assets/db/kmh_reports";
            if let Err(err) = fs::create_dir_all(dir_path) {
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_report_create: create_dir_all {dir_path}: {err:?}"
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_create: to_replay_msg вернул None (create_dir_all)");
              }
              return Ok(());
            }

            let file_path = format!("{}/{}.json", dir_path, report_id);

            let write_result: Result<(), _> = File::create(&file_path).and_then(|f| {
              serde_json::to_writer_pretty(f, &kmh_instance).map_err(std::io::Error::other)
            });

            if let Err(err) = write_result {
              let err = internal_error(action.name.clone(), None)
                .with_message(format!("kmh_report_create: write {:?}: {err:?}", file_path));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_create: to_replay_msg вернул None при ошибке записи файла");
              }
              return Ok(());
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_report_create: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== KMH_REPORT_CALC =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("kmh_report_calc")
            && action.args.is_some()
          {
            let raw_args = action.args.clone().unwrap();

            if let Ok(pretty) = serde_json::to_string_pretty(&raw_args) {
              info!("kmh_report_calc: сырые args из IPC:\n{}", pretty);
            } else {
              info!("kmh_report_calc: сырые args (Debug): {:?}", raw_args);
            }

            let mut kmh_instance =
              match serde_json::from_value::<KMHReportInstance>(raw_args.clone()) {
                Ok(v) => {
                  info!(
                    "kmh_report_calc: входной KMHReportInstance: id={:?}, title={:?}, status={:?}",
                    v.id, v.title, v.status
                  );

                  info!("kmh_report_calc: data ДО расчёта (Debug): {:?}", v.data);
                  if let Ok(pretty) = serde_json::to_string_pretty(&v.data) {
                    info!("kmh_report_calc: data ДО расчёта (JSON):\n{}", pretty);
                  }

                  v
                }
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_calc: {}", err));

                  info!("kmh_report_calc: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_calc: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "kmh_report_calc: расчёт КМХ для report_id={}",
              kmh_instance.id
            );

            let mut calc = KMHCalculator {
              report: kmh_instance.data.clone(),
            };
            let calculated_report = match calc.get_results() {
              Ok(calculated_report) => calculated_report,
              Err(err) => {
                let err =
                  internal_error(action.name.clone(), None).with_message(format!("${}", err));

                if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("kmh_report_calc: to_replay_msg вернул None (calculated_report)");
                }
                return Ok(());
              }
            };

            info!(
              "kmh_report_calc: результат расчёта (Debug) для report_id={}: {:#?}",
              kmh_instance.id, calculated_report
            );

            // if let Ok(pretty) = serde_json::to_string_pretty(&calculated_report) {
            //   info!(
            //     "kmh_report_calc: результат расчёта (JSON) для report_id={}:\n{}",
            //     kmh_instance.id, pretty
            //   );
            // } else {
            //   error!(
            //     "kmh_report_calc: не удалось сериализовать calculated_report в JSON для логов"
            //   );
            // }

            // if let Ok(pretty) = serde_json::to_string(&calculated_report) {
            //   match serde_json::from_str::<KMHReport>(&pretty) {
            //     Ok(_) => {
            //       info!("kmh_report_calc: самопроверка десериализации KMHReport прошла успешно");
            //     }
            //     Err(err) => {
            //       error!(
            //         "kmh_report_calc: САМОПРОВЕРКА провалилась: calculated_report уже сейчас не десериализуется в KMHReport: {err:?}"
            //       );
            //     }
            //   }
            // }

            let new_status = KmhIpcHandler::status_from_report(&calculated_report);
            info!(
              "kmh_report_calc: выставляем статус отчёта report_id={} => {:?}",
              kmh_instance.id, new_status
            );

            kmh_instance.status = new_status;
            kmh_instance.data = calculated_report;

            let dir_path = "assets/db/kmh_reports";
            if let Err(err) = fs::create_dir_all(dir_path) {
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_report_calc: create_dir_all {dir_path}: {err:?}"
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_report_calc: шлём ошибку в ipc_router (create_dir_all)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_calc: to_replay_msg вернул None (create_dir_all)");
              }
              return Ok(());
            }

            let file_path = format!("{}/{}.json", dir_path, kmh_instance.id);
            info!(
              "kmh_report_calc: сохраняем пересчитанный отчёт в файл: {}",
              file_path
            );

            let write_result: Result<(), _> = File::create(&file_path).and_then(|f| {
              serde_json::to_writer_pretty(f, &kmh_instance).map_err(std::io::Error::other)
            });

            if let Err(err) = write_result {
              let err = internal_error(action.name.clone(), None)
                .with_message(format!("kmh_report_calc: write {:?}: {err:?}", file_path));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_report_calc: шлём ошибку в ipc_router (write)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_calc: to_replay_msg вернул None при ошибке записи файла");
              }
              return Ok(());
            }

            if let Err(err) = export_kmh_report_to_xlsx(&kmh_instance) {
              error!(
                "kmh_report_calc: failed to export XLSX for report_id={}: {err:?}",
                kmh_instance.id
              );
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              info!("kmh_report_calc: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_report_calc: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== KMH_REPORT_SET =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("kmh_report_set")
            && action.args.is_some()
          {
            let raw_args = action.args.clone().unwrap();
            if let Ok(_args_pretty) = serde_json::to_string_pretty(&raw_args) {}

            let kmh_instance = match serde_json::from_value::<KMHReportInstance>(raw_args.clone()) {
              Ok(v) => v,
              Err(err) => {
                error!("KMH_REPORT_SET: ошибка парсинга KMHReportInstance из args: {err:?}");

                let err = internal_error(action.name.clone(), None)
                  .with_message(format!("kmh_report_set: cannot parse args: {err}"));

                if let Err(send_err) = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                {
                  error!(
                    "KMH_REPORT_SET: ошибка отправки ответа в ipc_router при bad args: {send_err:?}"
                  );
                }
                return Ok(());
              }
            };

            let dir_path = "assets/db/kmh_reports";

            if let Err(err) = fs::create_dir_all(dir_path) {
              error!("KMH_REPORT_SET: ошибка create_dir_all('{dir_path}'): {err:?}");
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_report_set: create_dir_all {dir_path}: {err:?}"
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                if let Err(send_err) = state.ipc_router.send_message(Some(msg)) {
                  error!(
                    "KMH_REPORT_SET: ошибка отправки ответа в ipc_router (create_dir_all): {send_err:?}"
                  );
                }
              } else {
                error!(
                  "KMH_REPORT_SET: to_replay_msg вернул None при ошибке create_dir_all('{dir_path}')"
                );
              }
              return Ok(());
            }

            let file_path = format!("{}/{}.json", dir_path, kmh_instance.id);

            let write_result: Result<(), _> = File::create(&file_path).and_then(|f| {
              serde_json::to_writer_pretty(f, &kmh_instance).map_err(std::io::Error::other)
            });

            if let Err(err) = write_result {
              error!(
                "KMH_REPORT_SET: ошибка записи файла отчёта {:?}: {err:?}",
                file_path
              );

              let err = internal_error(action.name.clone(), None)
                .with_message(format!("kmh_report_set: write {:?}: {err:?}", file_path));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                if let Err(send_err) = state.ipc_router.send_message(Some(msg)) {
                  error!(
                    "KMH_REPORT_SET: ошибка отправки ответа в ipc_router (write error): {send_err:?}"
                  );
                }
              } else {
                error!(
                  "KMH_REPORT_SET: to_replay_msg вернул None при ошибке записи файла {:?}",
                  file_path
                );
              }
              return Ok(());
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              if let Err(send_err) = state.ipc_router.send_message(Some(msg)) {
                error!(
                  "KMH_REPORT_SET: ошибка отправки успешного ответа в ipc_router: {send_err:?}"
                );
              }
            } else {
              error!(
                "KMH_REPORT_SET: to_replay_msg вернул None при формировании успешного ответа (report_id={:?})",
                kmh_instance.id
              );
            }

            return Ok(());
          }
        }
      }
    }

    Ok(())
  }
}

fn export_kmh_report_to_xlsx(
  instance: &KMHReportInstance,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  fn col_letter(mut idx: u32) -> String {
    let mut s = String::new();
    while idx > 0 {
      let rem = ((idx - 1) % 26) as u8;
      s.insert(0, (b'A' + rem) as char);
      idx = (idx - 1) / 26;
    }
    s
  }

  let template_path = Path::new("assets/report_tempplates/kmh_report.xlsx");
  if !template_path.exists() {
    return Err(format!("kmh_report template not found at {:?}", template_path).into());
  }

  let mut book = reader::xlsx::read(template_path)?;
  let sheet = book
    .get_sheet_mut(&0)
    .ok_or("kmh_report.xlsx has no sheets")?;

  let report = &instance.data;

  let created = instance.created_at.with_timezone(&Local);

  sheet
    .get_cell_mut("J2")
    .set_value(created.format("%d.%m.%Y").to_string());

  sheet
    .get_cell_mut("L2")
    .set_value(created.format("%H:%M").to_string());

  let tank_id = instance
    .title
    .clone()
    .unwrap_or_else(|| format!("КМХ отчёт {}", instance.id).into());

  sheet.get_cell_mut("C3").set_value(tank_id);
  sheet.get_cell_mut("C4").set_value("ikm");

  sheet
    .get_cell_mut("F8")
    .set_value(report.air_temperature_outside.to_string());
  sheet
    .get_cell_mut("F9")
    .set_value(report.air_pressure_outside.to_string());
  sheet
    .get_cell_mut("F10")
    .set_value(report.wind_speed.to_string());

  let tape_class_val: i32 = match report.tape_class {
    TapeClass::One => 1,
    TapeClass::Two => 2,
    TapeClass::Three => 3,
  };
  sheet
    .get_cell_mut("K14")
    .set_value(tape_class_val.to_string());

  sheet
    .get_cell_mut("K15")
    .set_value(report.ruler_alpha_coefficient.to_string());

  sheet
    .get_cell_mut("K16")
    .set_value(report.wall_alpha_coefficient.to_string());

  sheet
    .get_cell_mut("K17")
    .set_value(report.nominal_height.to_string());

  sheet
    .get_cell_mut("K18")
    .set_value(report.air_temp_verify.to_string());

  sheet
    .get_cell_mut("C29")
    .set_value(report.vapor_temp.to_string());

  sheet
    .get_cell_mut("B29")
    .set_value(report.measured_height.to_string());

  for (idx, (high, low)) in report.gas_layer_height_measured_points.iter().enumerate() {
    if idx >= 4 {
      break;
    }
    let row: u32 = 23;
    let col_high = 2 + (idx as u32) * 2;
    let col_low = col_high + 1;

    let addr_high = format!("{}{}", col_letter(col_high), row);
    let addr_low = format!("{}{}", col_letter(col_low), row);

    sheet.get_cell_mut(&*addr_high).set_value(high.to_string());
    sheet.get_cell_mut(&*addr_low).set_value(low.to_string());
  }

  for (idx, sensor) in report.temperature_channels.iter().enumerate() {
    if idx >= 10 {
      break;
    }
    let row: u32 = 34 + idx as u32;

    let addr_level = format!("B{}", row);
    let addr_t_meas = format!("C{}", row);
    let addr_t_ctrl = format!("E{}", row);

    sheet
      .get_cell_mut(&*addr_level)
      .set_value(sensor.level.to_string());

    sheet
      .get_cell_mut(&*addr_t_meas)
      .set_value(sensor.temperature.to_string());

    sheet
      .get_cell_mut(&*addr_t_ctrl)
      .set_value(sensor.temperature_controlled.to_string());
  }

  sheet
    .get_cell_mut("C48")
    .set_value(report.density_measured.to_string());

  sheet.get_cell_mut("E48").set_value(
    report
      .density_measured_controlled
      .0
      .map_or("".to_string(), |v| v.to_string()),
  );
  sheet.get_cell_mut("E49").set_value(
    report
      .density_measured_controlled
      .1
      .map_or("".to_string(), |v| v.to_string()),
  );
  sheet.get_cell_mut("E50").set_value(
    report
      .density_measured_controlled
      .2
      .map_or("".to_string(), |v| v.to_string()),
  );

  sheet
    .get_cell_mut("K54")
    .set_value(report.density_verified.to_string());

  sheet
    .get_cell_mut("K53")
    .set_value(report.pontoon_mass.to_string());

  sheet
    .get_cell_mut("B57")
    .set_value(report.product_volume_measured.to_string());

  sheet
    .get_cell_mut("E57")
    .set_value(report.volume_coarse.to_string());

  sheet
    .get_cell_mut("I57")
    .set_value(report.delta_v_max.to_string());

  sheet
    .get_cell_mut("B62")
    .set_value(report.product_mass_measured.to_string());

  sheet
    .get_cell_mut("G62")
    .set_value(report.delta_m_max.to_string());

  let out_path = format!("assets/downloads/{}.xlsx", instance.id);
  writer::xlsx::write(&book, &out_path)?;

  Ok(())
}
