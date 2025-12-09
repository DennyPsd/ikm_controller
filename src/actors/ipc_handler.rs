use std::collections::HashMap;
use std::fs::{self, File};

use chrono::Local;
use taxon_core::actors::ipc::errors::internal_error;
use taxon_core::infrastructure::facility::DataChange;

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
          // ===================== TankConfigSet =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("tank_config_set")
            && action.args.is_some()
          {
            let args = match serde_json::from_value::<TankConfig>(action.args.clone().unwrap()) {
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
            let config_vars_path = format!(
              "assets/db/tanks/{}/config.yaml",
              action.target.device_id.unwrap()
            );

            let config_content = fs::read_to_string(&config_vars_path)?;
            let old_config: TankConfig = serde_saphyr::from_str(&config_content)
              .map_err(|err| format!("Cant parse config: {err:?}"))?;
            let new_config = args;
            let change =
              DataChange::generate_changes(&old_config, &new_config, "admin".into(), Local::now());

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
