use crate::actors::tank_calc::Meta;
use crate::types::kmh::{KMHReportInstance, KMHReportStatus};
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Tank};
use crate::types::type_traits::KMHReportExt;
use crate::{KMHReportCreateArgs, KMHReportListArgs, KMHReportListFields, KMHReportListReply};
use chrono::Local;
use ikm_calc::calculation::kmh::{KMHCalculator, KMHReport, TapeClass};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;
use smol_str::SmolStr;
use std::fs::{self, File};
use taxon_core::actors::ipc::errors::internal_error;
use taxon_core::infrastructure::facility::SharedData;
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tracing::{error, info};
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
      delta_height: 0.0,

      // Плотности
      density_verified: 0.0,
      density_measured: 0.0,
      density_measured_controlled: (0.0, 0.0, 0.0),

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
            info!("KmhIpcHandler: обработка action 'kmh_report_list'");

            let args =
              match serde_json::from_value::<KMHReportListArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_list: {}", err));

                  info!("kmh_report_list: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_list: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "kmh_report_list: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );

            let dir_path = "assets/db/kmh_reports";
            let mut reports: Vec<KMHReportInstance> = Vec::new();

            if args.ids.is_empty() {
              // читаем все файлы из директории kmh_reports
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
                  // если директории нет — считаем, что просто ещё нет отчётов
                  if err.kind() != std::io::ErrorKind::NotFound {
                    let err = internal_error(action.name.clone(), None)
                      .with_message(format!("kmh_report_list: read_dir {dir_path}: {err:?}"));

                    if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                      info!("kmh_report_list: шлём ошибку в ipc_router (read_dir)");
                      let _ = state.ipc_router.send_message(Some(msg));
                    } else {
                      error!("kmh_report_list: to_replay_msg вернул None при ошибке read_dir");
                    }
                    return Ok(());
                  }
                }
              }
            } else {
              // ids трактуем как id отчётов
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
                  Err(err) => {
                    info!("kmh_report_list: нет файла отчёта {} ({err:?})", file_path);
                  }
                }
              }
            }

            // Приводим к нужному уровню детализации
            let reports = match args.fields {
              KMHReportListFields::Minimal => {
                // Minimal: возвращаем только «шапку» отчёта.
                // Поле `data` обнуляем, `tank` гарантированно делаем Link.
                reports
                  .into_iter()
                  .map(|mut r| {
                    r.data = KmhIpcHandler::empty_kmh_report();
                    r.tank = r.tank.into_link_sync();
                    r
                  })
                  .collect::<Vec<_>>()
              }
              KMHReportListFields::All => {
                // All: возвращаем всё как есть
                reports
              }
              KMHReportListFields::Exact(_fields) => {
                // TODO: тонкая выборка полей по списку `fields`.
                // Пока ведём себя как All.
                reports
              }
            };

            info!("kmh_report_list: найдено {} отчётов", reports.len());

            let reply = KMHReportListReply { data: reports };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(reply)), None) {
              info!("kmh_report_list: отправляем ответ в ipc_router");
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
            info!("KmhIpcHandler: обработка action 'kmh_report_create'");

            // 0. Парсим аргументы: { device_id: Uuid }
            let args =
              match serde_json::from_value::<KMHReportCreateArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_create: {}", err));

                  info!("kmh_report_create: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_create: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "kmh_report_create: создание болванки отчёта для tank/device_id={}",
              args.device_id
            );

            // 1. Читаем tanks.yaml и ищем нужный танк
            let tanks_path = "assets/db/tanks.yaml";
            let tanks: Vec<Tank> = match File::open(tanks_path)
              .map_err(|err| {
                internal_error(action.name.clone(), None).with_message(format!("{err:?}"))
              })
              .and_then(|reader| {
                serde_saphyr::from_reader::<File, Vec<Tank>>(reader).map_err(|err| {
                  internal_error(action.name.clone(), None).with_message(format!("{err:?}"))
                })
              }) {
              Ok(data) => data,
              Err(err) => {
                if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                  info!("kmh_report_create: шлём ошибку в ipc_router (read/parse tanks.yaml)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!(
                    "kmh_report_create: to_replay_msg вернул None при ошибке чтения tanks.yaml"
                  );
                }
                return Ok(());
              }
            };

            let tank = match tanks.into_iter().find(|t| t.id == args.device_id) {
              Some(t) => t,
              None => {
                let err = internal_error(action.name.clone(), None).with_message(format!(
                  "kmh_report_create: tank with id={} not found",
                  args.device_id
                ));

                if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                  info!("kmh_report_create: шлём ошибку в ipc_router (tank not found)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("kmh_report_create: to_replay_msg вернул None (tank not found)");
                }
                return Ok(());
              }
            };

            // 2. Подтягиваем конфиг + base_vars + ext_vars + meta для этого танка
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
              Err(err) => {
                info!(
                  "kmh_report_create: нет config для {} ({err:?})",
                  args.device_id
                );
                None
              }
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
              Err(err) => {
                info!(
                  "kmh_report_create: нет base_vars для {} ({err:?})",
                  args.device_id
                );
                None
              }
            };

            let ext_vars: Option<ExtVars> = match File::open(&ext_vars_path) {
              Ok(f) => match serde_saphyr::from_reader::<File, ExtVars>(f) {
                Ok(v) => Some(v),
                Err(err) => {
                  error!(
                    "kmh_report_create: не удалось распарсить ext_vars {}: {err:?}",
                    ext_vars_path
                  );
                  None
                }
              },
              Err(err) => {
                info!(
                  "kmh_report_create: нет ext_vars для {} ({err:?})",
                  args.device_id
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
              Err(err) => {
                info!(
                  "kmh_report_create: нет meta для calc/{} ({err:?})",
                  args.device_id
                );
                None
              }
            };

            // утилита для парсинга f64 из Option<String> (с запятой/точкой)
            let _parse_opt_f64 = |s: &Option<String>| {
              s.as_ref()
                .and_then(|v| v.replace(',', ".").parse::<f64>().ok())
            };

            // 3. Базовая «болванка» KMHReport
            let mut kmh_report = KMHReport {
              // Окружение
              air_temperature_outside: 0.0,
              air_pressure_outside: 0.0,
              wind_speed: 0.0,
              gas_layer_height_measured_points: Vec::new(),
              measured_height: 0.0,
              nominal_height: 0.0,
              delta_height: 0.0,

              // Плотности
              density_verified: 0.0,
              density_measured: 0.0,
              density_measured_controlled: (0.0, 0.0, 0.0),

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
            };

            // 3.1. Если есть meta.json — обогащаем через Constants → KMHReport
            if let Some(m) = &meta {
              kmh_report.apply_constants(&m.constants);
            }

            // 3.2. Если есть config.yaml — переопределяем то, что явно задано в конфиге
            if let Some(cfg) = &config {
              // HБ — базовая высота резервуара
              if let Some(h) = &cfg.basic_data.basic_height {
                kmh_report.nominal_height = *h as f64;
              }

              // αст — линейное расширение стенки резервуара
              if let Some(a) = &cfg.construction.linear_expansion {
                kmh_report.wall_alpha_coefficient = *a as f64;
              }

              // m(понтона) — масса понтона
              if let Some(m) = &cfg.construction.mass_floating_coating {
                kmh_report.pontoon_mass = *m as f64;
              }

              // ΔH — предел абсолютной погрешности измерения уровня
              if let Some(dh) = &cfg
                .measurement_accuracy_indicators
                .limit_permissible_absolute_measurement_reservoir_level
              {
                kmh_report.delta_height = *dh as f64;
              }
              // Температура воздуха при поверке резервуара
              if let Some(dh) = &cfg.basic_data.air_temp_verify {
                kmh_report.air_temp_verify = *dh as f64;
              }

              // Остальные поля KMHReport из конфига пока не трогаем — оператор + расчёт.
            }

            // 3.3. Если есть base_vars + ext_vars — обогащаем измерениями
            if let (Some(base), Some(ext)) = (&base_vars, &ext_vars) {
              kmh_report.apply_base_ext(base, ext);
            }

            // 4. DataLink<Tank> через SharedData::new_link_to()
            let tank_link = tank.new_link_to();

            // 5. Собираем KMHReportInstance
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

            // 6. Создаём директорию и пишем файл assets/db/kmh_reports/{id}.json
            let dir_path = "assets/db/kmh_reports";
            if let Err(err) = fs::create_dir_all(dir_path) {
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_report_create: create_dir_all {dir_path}: {err:?}"
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_report_create: шлём ошибку в ipc_router (create_dir_all)");
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
                info!("kmh_report_create: шлём ошибку в ipc_router (write)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_create: to_replay_msg вернул None при ошибке записи файла");
              }
              return Ok(());
            }

            // 7. Отправляем обратно уже заполненный KMHReportInstance
            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              info!(
                "kmh_report_create: отправляем созданный отчёт id={} в ipc_router",
                report_id
              );
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
            info!("KmhIpcHandler: обработка action 'kmh_report_calc'");

            let mut kmh_instance =
              match serde_json::from_value::<KMHReportInstance>(action.args.clone().unwrap()) {
                Ok(v) => v,
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

            // 1. Чистый пересчёт
            let mut calc = KMHCalculator {
              report: kmh_instance.data.clone(),
            };
            let calculated_report = calc.get_results();
            kmh_instance.data = calculated_report;

            // 2. Сохраняем на диск в assets/db/kmh_reports/{id}.json
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

            // 3. Отправляем обратно уже пересчитанный и сохранённый инстанс
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
            info!("KmhIpcHandler: обработка action 'kmh_report_set'");

            let kmh_instance =
              match serde_json::from_value::<KMHReportInstance>(action.args.clone().unwrap()) {
                Ok(v) => v,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_report_set: {}", err));

                  info!("kmh_report_set: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_report_set: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "kmh_report_set: сохранение КМХ без пересчёта для report_id={}",
              kmh_instance.id
            );

            let dir_path = "assets/db/kmh_reports";
            if let Err(err) = fs::create_dir_all(dir_path) {
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_report_set: create_dir_all {dir_path}: {err:?}"
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_report_set: шлём ошибку в ipc_router (create_dir_all)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_set: to_replay_msg вернул None (create_dir_all)");
              }
              return Ok(());
            }

            let file_path = format!("{}/{}.json", dir_path, kmh_instance.id);

            let write_result: Result<(), _> = File::create(&file_path).and_then(|f| {
              serde_json::to_writer_pretty(f, &kmh_instance).map_err(std::io::Error::other)
            });

            if let Err(err) = write_result {
              let err = internal_error(action.name.clone(), None)
                .with_message(format!("kmh_report_set: write {:?}: {err:?}", file_path));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_report_set: шлём ошибку в ipc_router (write)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_report_set: to_replay_msg вернул None при ошибке записи файла");
              }
              return Ok(());
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              info!("kmh_report_set: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_report_set: to_replay_msg вернул None");
            }

            return Ok(());
          }

          info!(
            "KmhIpcHandler: непонятный action для kmh: name={:?}, kind={:?} — игнорируем",
            action.name, action.kind
          );
        }
      }
    }

    Ok(())
  }
}
