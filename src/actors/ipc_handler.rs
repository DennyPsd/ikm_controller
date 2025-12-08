use std::collections::HashMap;
use std::fs::File;

use taxon_core::actors::ipc::errors::internal_error;

use crate::actors::ipc_kmh_handler::KmhIpcHandlerMsg;
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;
use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Tank};
use crate::{ProductListArgs, TankListArgs};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::{json, to_string_pretty};
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
#[allow(dead_code)]
pub struct Subscriber {
  pub template: IPCMessageCrate,
}

pub struct IpcHandlerState {
  #[allow(dead_code)]
  pub hart_fabric: ActorRef<ModbusFabricMsg>,
  #[allow(dead_code)]
  pub ipc_router: ActorRef<Option<IPCActorMsg>>,
  #[allow(dead_code)]
  pub subscribers: Vec<Subscriber>,

  // новый актор для kmh
  #[allow(dead_code)]
  pub kmh_handler: ActorRef<KmhIpcHandlerMsg>,
}

#[derive(Debug)]
pub enum IpcHandlerMsg {
  Ipc(IPCMessageCrate),
}

pub struct IpcHandler;

#[ractor::async_trait]
impl Actor for IpcHandler {
  type Msg = IpcHandlerMsg;
  type State = IpcHandlerState;
  type Arguments = IpcHandlerState;

  async fn pre_start(
    &self,
    _myself: ActorRef<Self::Msg>,
    args: Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    info!("Modbus IPC_Handler запущен");
    Ok(args)
  }

  async fn handle(
    &self,
    _myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut Self::State,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      IpcHandlerMsg::Ipc(ipc_msg) => {
        info!(
          "Modbus IPC_Handler: получено IPC сообщение: peer={:?}, protocol={:?}",
          ipc_msg.peer_from, ipc_msg.protocol
        );
        if let Ok(s) = to_string_pretty(&ipc_msg.msg) {
          info!("Modbus IPC_Handler: raw msg =\n{}", s);
        }

        // сначала проверяем, kmh ли это — если да, просто форвардим в kmh-актор и выходим
        if let Some(action) = ipc_msg.as_action() {
          let is_kmh = (action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("kmh_report_list"))
            || (action.kind == IPCActionKind::SetData && action.name.as_deref() == Some("kmh_set"));

          if is_kmh {
            info!("Modbus IPC_Handler: форвардим kmh_* в KmhIpcHandler");
            let _ = state
              .kmh_handler
              .send_message(KmhIpcHandlerMsg::Ipc(ipc_msg));
            return Ok(());
          }
        }

        // дальше вся остальная логика как раньше (tank_list, products_list, …)
        if let Some(action) = ipc_msg.as_action() {
          info!(
            "Modbus IPC_Handler: action.name={:?}, kind={:?}, target.module_name={:?}, data_ns={:?}",
            action.name, action.kind, action.target.module_name, action.target.data_ns,
          );

          // ===================== TANK_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("tank_list")
            && action.args.is_some()
          {
            let args = match serde_json::from_value::<TankListArgs>(action.args.clone().unwrap()) {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone(), None)
                  .with_message(format!("tank_list: {}", err));

                info!("tank_list: шлём ошибку в ipc_router (read_to_string)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("tank_list: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let path = "assets/db/tanks.yaml";
            let mut tanks: HashMap<_, _> = match File::open(path)
              .map_err(|err| {
                internal_error(action.name.clone(), None).with_message(format!("{:?}", err))
              })
              .and_then(|reader| {
                serde_saphyr::from_reader::<File, Vec<Tank>>(reader)
                  .map_err(|err| {
                    internal_error(action.name.clone(), None).with_message(format!("{:?}", err))
                  })
                  .map(|v| v.into_iter().map(|v| (v.id, v)).collect())
              }) {
              Ok(data) => data,
              Err(err) => {
                if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                  info!("tank_list: шлём ошибку в ipc_router (read_to_string)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("tank_list: to_replay_msg вернул None при ошибке чтения файла");
                }
                return Ok(());
              }
            };

            let ids: Vec<_> = if !args.ids.is_empty() {
              args.ids.to_vec()
            } else {
              tanks.keys().cloned().collect()
            };

            for id in ids.iter() {
              let tank = tanks.get_mut(id).unwrap();
              match args.fields {
                crate::TankListFields::Minimal => {
                  let path = format!("assets/db/tanks/{id}/base_vars.yaml");
                  tank.base_vars = File::open(&path[..])
                    .map_err(|err| {
                      error!("{err:#?}");
                      Option::<()>::None
                    })
                    .map_or(None, |reader| {
                      serde_saphyr::from_reader::<File, BaseVars>(reader)
                        .map_err(|err| {
                          error!("{err:#?}");
                          Option::<()>::None
                        })
                        .ok()
                    });
                }
                _ => {
                  let path = format!("assets/db/tanks/{id}/base_vars.yaml");
                  tank.base_vars = File::open(&path[..])
                    .map_err(|err| {
                      error!("{err:#?}");
                      Option::<()>::None
                    })
                    .map_or(None, |reader| {
                      serde_saphyr::from_reader::<File, BaseVars>(reader)
                        .map_err(|err| {
                          error!("{err:#?}");
                          Option::<()>::None
                        })
                        .ok()
                    });

                  let path = format!("assets/db/tanks/{id}/config.yaml");
                  tank.config = File::open(&path[..])
                    .map_err(|err| {
                      error!("{err:#?}");
                      Option::<()>::None
                    })
                    .map_or(None, |reader| {
                      serde_saphyr::from_reader::<File, TankConfig>(reader)
                        .map_err(|err| {
                          error!("{err:#?}");
                          Option::<()>::None
                        })
                        .ok()
                    });

                  let path = format!("assets/db/tanks/{id}/ext_vars.yaml");
                  tank.ext_vars = File::open(&path[..])
                    .map_err(|err| {
                      error!("{err:#?}");
                      Option::<()>::None
                    })
                    .map_or(None, |reader| {
                      serde_saphyr::from_reader::<File, ExtVars>(reader)
                        .map_err(|err| {
                          error!("{err:#?}");
                          Option::<()>::None
                        })
                        .ok()
                    });
                }
              }
            }

            let data = tanks
              .into_iter()
              .filter_map(|(id, v)| if ids.contains(&id) { Some(v) } else { None })
              .collect::<Vec<_>>();

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("tank_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("tank_list: to_replay_msg вернул None");
            }
            return Ok(());
          }

          // ===================== PRODUCTS_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("products_list")
            && action.args.is_some()
          {
            info!("Modbus IPC_Handler: обработка action 'products_list'");

            let args = match serde_json::from_value::<ProductListArgs>(action.args.clone().unwrap())
            {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone(), None)
                  .with_message(format!("products_list: {}", err));

                info!("products_list: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("products_list: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            info!(
              "products_list: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );

            let path = "assets/db/tanks.yaml";
            let tanks: Vec<Tank> = match File::open(path)
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
                  info!("products_list: шлём ошибку в ipc_router (read/parse)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("products_list: to_replay_msg вернул None при ошибке чтения файла");
                }
                return Ok(());
              }
            };

            let mut products_by_id: HashMap<Uuid, Product> = HashMap::new();

            for tank in tanks.into_iter() {
              if let Some(prod) = tank.product {
                let prod_id = match &prod {
                  Product::Oil { id, .. } => *id,
                  Product::OilProduct { id, .. } => *id,
                };

                products_by_id.entry(prod_id).or_insert(prod);
              }
            }

            let ids: Vec<Uuid> = if !args.ids.is_empty() {
              args.ids.clone()
            } else {
              products_by_id.keys().cloned().collect()
            };

            let data: Vec<Product> = ids
              .into_iter()
              .filter_map(|id| products_by_id.get(&id).cloned())
              .collect();

            info!("products_list: найдено {} продуктов", data.len());

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("products_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("products_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== KMH_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("kmh_report_list")
            && action.args.is_some()
          {
            info!("Modbus IPC_Handler: обработка action 'kmh_report_list'");

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
            let tank_ids: Vec<Uuid> = if !args.ids.is_empty() {
              args.ids.clone()
            } else {
              let path = "assets/db/tanks.yaml";
              let tanks: Vec<Tank> = match File::open(path)
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
                    info!("kmh_report_list: шлём ошибку в ipc_router (read tanks.yaml)");
                    let _ = state.ipc_router.send_message(Some(msg));
                  } else {
                    error!(
                      "kmh_report_list: to_replay_msg вернул None при ошибке чтения tanks.yaml"
                    );
                  }
                  return Ok(());
                }
              };

              tanks.into_iter().map(|t| t.id).collect()
            };

            let mut reports: Vec<KMHReportInstance> = Vec::new();

            for tank_id in tank_ids {
              let file_path = format!("assets/db/calc/{tank_id}/kmh_report.json");

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
                  info!("kmh_report_list: нет файла {} ({err:?})", file_path);
                }
              }
            }

            info!("kmh_report_list: найдено {} отчётов", reports.len());

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": reports })), None) {
              info!("kmh_report_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_report_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== KMH_SET =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("kmh_set")
            && action.args.is_some()
          {
            info!("Modbus IPC_Handler: обработка action 'kmh_set'");

            let mut kmh_instance =
              match serde_json::from_value::<KMHReportInstance>(action.args.clone().unwrap()) {
                Ok(v) => v,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
                    .with_message(format!("kmh_set: {}", err));

                  info!("kmh_set: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("kmh_set: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "kmh_set: расчёт КМХ для device_id={}, report_id={}",
              kmh_instance.device.id, kmh_instance.id
            );

            let mut calc = KMHCalculator {
              report: kmh_instance.data.clone(),
            };
            let calculated_report = calc.get_results();

            kmh_instance.data = calculated_report;

            let dir_path = format!("assets/db/calc/{}", kmh_instance.device_id);
            if !Path::new(&dir_path).is_dir() {
              let err = internal_error(action.name.clone(), None).with_message(format!(
                "kmh_set: директории с метой не существует: {}",
                dir_path
              ));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_set: шлём ошибку в ipc_router (no calc dir)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_set: to_replay_msg вернул None при ошибке (no calc dir)");
              }
              return Ok(());
            }

            let file_path = format!("{}/kmh_report.json", dir_path);

            let write_result: Result<(), _> = File::create(&file_path).and_then(|f| {
              serde_json::to_writer_pretty(f, &kmh_instance).map_err(std::io::Error::other)
            });

            if let Err(err) = write_result {
              let err = internal_error(action.name.clone(), None)
                .with_message(format!("kmh_set: write {:?}: {err:?}", file_path));

              if let Some(msg) = ipc_msg.to_replay_msg(Option::<()>::None, Some(err)) {
                info!("kmh_set: шлём ошибку в ipc_router (write)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("kmh_set: to_replay_msg вернул None при ошибке записи файла");
              }
              return Ok(());
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(kmh_instance)), None) {
              info!("kmh_set: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("kmh_set: to_replay_msg вернул None");
            }

            return Ok(());
          }

          info!(
            "Modbus IPC_Handler: непонятный action: name={:?}, kind={:?} — игнорируем",
            action.name, action.kind
          );
        }
      }
    }

    Ok(())
  }
}
