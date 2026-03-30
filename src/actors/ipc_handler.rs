use crate::actors::ipc_kmh_handler::KmhIpcHandlerMsg;
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;
use crate::msges::data_change_list::DataChangeArgs;
use crate::msges::event_list::EventListArgs;
use crate::msges::event_rule_create::EventRuleCreateArgs;
use crate::msges::event_rule_list::EventRuleListArgs;
use crate::msges::load_grad_table::LoadGradTableArgs;
use crate::msges::product_create::ProductCreateArgs;
use crate::msges::product_list::ProductListArgs;
use crate::msges::tank_list::{TankListArgs, TankListFields};
use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Tank};
use base64::Engine;
use base64::engine::general_purpose;
use chrono::Local;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::{json, to_string_pretty};
use std::collections::HashMap;
use std::fs::{self, File};
use taxon_core::components::data::{DataChange, DataLink, DataModel};
use taxon_core::components::device::{FacilityEvent, FacilityEventRule};
use taxon_core::ipc::errors::internal_error;
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tracing::{debug, error, info, warn};
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
                let err =
                  internal_error(action.name.clone()).with_message(format!("tank_list: {}", err));

                debug!("tank_list: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("tank_list: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            // грузим все танки
            let mut tanks: HashMap<_, _> = Tank::load_list()
              .await
              .into_iter()
              .map(|v| (v.id, v))
              .collect();

            // грузим все продукты один раз и кладём в map по id
            let products_by_id: HashMap<Uuid, Product> = Product::load_list()
              .await
              .into_iter()
              .map(|p| (p.id().clone(), p))
              .collect();

            let ids: Vec<_> = if !args.ids.is_empty() {
              args.ids.clone()
            } else {
              tanks.keys().cloned().collect()
            };
            info!("tanks ids: {tanks:#?}");
            for id in ids.iter() {
              let tank = tanks.get_mut(id).unwrap();

              match args.fields {
                TankListFields::Minimal => {
                  // Tank::ge
                  // let path = format!("assets/db/tanks/{id}/base_vars.yaml");
                  // tank.base_vars = File::open(&path[..])
                  //   .map_err(|err| {
                  //     error!("{err:#?}");
                  //     Option::<()>::None
                  //   })
                  //   .map_or(None, |reader| {
                  //     serde_saphyr::from_reader::<File, BaseVars>(reader)
                  //       .map_err(|err| {
                  //         error!("{err:#?}");
                  //         Option::<()>::None
                  //       })
                  //       .ok()
                  //   });
                  // tank.base_vars = Tank::ge
                  // В Minimal product остаётся как Link (не раскрываем)
                }
                // All и прочие варианты — грузим всё + раскрываем product
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

                  // 🔗 раскрываем product: Link -> Data(Product)
                  if let DataLink::Link { id: prod_id, .. } = &tank.product {
                    if let Some(prod) = products_by_id.get(prod_id) {
                      tank.product = DataLink::Data(prod.clone());
                    }
                  }
                }
              }
            }

            let data = if ids.len() > 0 {
              tanks
                .into_iter()
                .filter_map(|(id, v)| if ids.contains(&id) { Some(v) } else { None })
                .collect::<Vec<_>>()
            } else {
              tanks.into_iter().map(|(id, v)| v).collect::<Vec<_>>()
            };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("tank_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("tank_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== TankConfigSet =====================

          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("tank_config_set")
            && action.args.is_some()
          {
            info!(
              "tank_config_set: enter kind={:?} name={:?} target={:?} has_args={}",
              action.kind,
              action.name,
              action.target,
              action.args.is_some()
            );

            // 1) device_id без panic
            let device_id = match action.target.device_id {
              Some(id) => id,
              None => {
                error!("tank_config_set: device_id is None");
                let err = internal_error(action.name.clone())
                  .with_message("tank_config_set: device_id is None".to_string());

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e}");
                }
                return Ok(());
              }
            };

            // 2) args без panic
            let args_value = match action.args.clone() {
              Some(v) => v,
              None => {
                error!("tank_config_set: args is None");
                let err = internal_error(action.name.clone())
                  .with_message("tank_config_set: args is None".to_string());

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e}");
                }
                return Ok(());
              }
            };

            // 3) parse json args
            let new_config: TankConfig = match serde_json::from_value(args_value) {
              Ok(v) => {
                debug!("tank_config_set: args parsed OK");
                v
              }
              Err(err) => {
                error!("tank_config_set: args parse error: {err}");
                let err = internal_error(action.name.clone())
                  .with_message(format!("tank_config_set: args parse error: {err}"));

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e}");
                }
                return Ok(());
              }
            };

            // 4) read old config
            let config_vars_path = format!("assets/db/tanks/{}/config.yaml", device_id);
            info!("tank_config_set: reading file: {}", config_vars_path);

            let config_content = match fs::read_to_string(&config_vars_path) {
              Ok(s) => s,
              Err(e) => {
                error!("tank_config_set: read_to_string failed: {e}");
                let err = internal_error(action.name.clone())
                  .with_message(format!("tank_config_set: read error: {e}"));

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e2) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e2}");
                }
                return Ok(());
              }
            };

            let old_config: TankConfig = match serde_saphyr::from_str(&config_content) {
              Ok(v) => v,
              Err(err) => {
                error!("tank_config_set: yaml parse failed: {err:?}");
                let err = internal_error(action.name.clone())
                  .with_message(format!("tank_config_set: cant parse config.yaml: {err:?}"));

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e2) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e2}");
                }
                return Ok(());
              }
            };

            // 5) changes
            info!("tank_config_set: generating changes...");
            let changes = DataChange::generate_changes(
              &old_config,
              &new_config,
              "admin".into(),
              Local::now(),
              device_id,
              new_config
                .basic_data
                .title
                .clone()
                .unwrap_or_default()
                .into(),
              "Tank".into(),
            );

            info!("tank_config_set: changes count={}", changes.len());
            debug!("tank_config_set: changes={:#?}", changes);

            if !changes.is_empty() {
              match DataChange::save_many(changes).await {
                Ok(_) => info!("tank_config_set: changes inserted OK"),
                Err(e) => warn!("tank_config_set: save_many failed (still continue): {e:?}"),
              }
            }

            // 6) write new config
            let yaml_out = match serde_saphyr::to_string(&new_config) {
              Ok(s) => s,
              Err(e) => {
                error!("tank_config_set: to_string failed: {e:?}");
                let err = internal_error(action.name.clone())
                  .with_message(format!("tank_config_set: serialize error: {e:?}"));

                let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
                if let Err(e2) = state.ipc_router.send_message(msg) {
                  error!("tank_config_set: failed to send error reply: {e2}");
                }
                return Ok(());
              }
            };

            info!("tank_config_set: writing file...");
            if let Err(e) = fs::write(&config_vars_path, yaml_out) {
              error!("tank_config_set: write failed: {e:?}");
              let err = internal_error(action.name.clone())
                .with_message(format!("tank_config_set: write error: {e:?}"));

              let msg = ipc_msg.to_replay_msg(Option::<()>::None, Some(err));
              if let Err(e2) = state.ipc_router.send_message(msg) {
                error!("tank_config_set: failed to send error reply: {e2}");
              }
              return Ok(());
            }

            // 7) ВОТ ЭТОГО РАНЬШЕ НЕ БЫЛО: success reply
            info!("tank_config_set: sending success reply...");
            let msg = ipc_msg.to_replay_msg(Some(()), None); // или Some(new_config.clone()), если ожидаешь данные
            if let Err(e) = state.ipc_router.send_message(msg) {
              error!("tank_config_set: failed to send success reply: {e}");
            } else {
              info!("tank_config_set: success reply sent");
            }

            return Ok(());
          }

          // ===================== PRODUCTS_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("product_list")
            && action.args.is_some()
          {
            info!("Modbus IPC_Handler: обработка action 'product_list'");

            let args = match serde_json::from_value::<ProductListArgs>(action.args.clone().unwrap())
            {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone())
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
              "products_list: входные аргументы: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );
            let products = Product::load_list().await;

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": products })), None) {
              info!("products_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("products_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== PRODUCT_CREATE =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("product_create")
          {
            info!("product_create: обработка запроса");

            if action.args.is_none() {
              let err =
                internal_error(action.name.clone()).with_message("product_create: empty args");

              info!("product_create: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_create: to_replay_msg {}", err);
                });
              return Ok(());
            }

            let create_args: ProductCreateArgs =
              match serde_json::from_value(action.args.clone().unwrap()) {
                Ok(p) => p,
                Err(err) => {
                  let err = internal_error(action.name.clone())
                    .with_message(format!("product_create: {}", err));

                  info!("product_create: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("product_create: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            let new_id = Uuid::now_v7();

            let product: Product = match create_args {
              ProductCreateArgs::Oil {
                title,
                vapor,
                product_weight,
                volume_at_15,
              } => Product::Oil {
                id: new_id,
                title,
                vapor,
                product_weight,
                volume_at_15,
              },
              ProductCreateArgs::OilProduct {
                title,
                vapor,
                product_weight,
                volume_at_15,
              } => Product::OilProduct {
                id: new_id,
                title,
                vapor,
                product_weight,
                volume_at_15,
              },
            };

            info!("product_create: присвоен новый id = {}", new_id);

            let mut products = Product::load_list().await;
            products.push(product.clone());

            if let Err(err) = Product::save_many(products).await {
              let err = internal_error(action.name.clone())
                .with_message(format!("product_create: save_many error: {err:?}"));

              error!("product_create: ошибка записи Products.yaml: {err:?}");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_create: to_replay_msg {}", err);
                });
              return Ok(());
            }

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(product)), None) {
              info!("product_create: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("product_create: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== PRODUCT_SET =====================
          if action.kind == IPCActionKind::SetData && action.name.as_deref() == Some("product_set")
          {
            info!("product_set: обработка запроса");

            if action.args.is_none() {
              let err = internal_error(action.name.clone()).with_message("product_set: empty args");

              info!("product_set: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            // парсим Product
            let product: Product = match serde_json::from_value(action.args.clone().unwrap()) {
              Ok(p) => p,
              Err(err) => {
                let err =
                  internal_error(action.name.clone()).with_message(format!("product_set: {}", err));

                info!("product_set: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("product_set: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let id = *product.id();
            info!("product_set: входящий product.id = {}", id);

            if id.is_nil() {
              let err = internal_error(action.name.clone())
                .with_message("product_set: id is nil, используйте product_create для создания");

              info!("product_set: шлём ошибку — пустой id");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            // грузим список, ищем продукт по id
            let mut products = Product::load_list().await;

            if let Some(pos) = products.iter().position(|p| p.id() == &id) {
              info!("product_set: обновляем существующий продукт {}", id);
              products[pos] = product.clone();
            } else {
              let err = internal_error(action.name.clone())
                .with_message(format!("product_set: product with id {id} not found"));

              error!("product_set: продукт с id {id} не найден");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            if let Err(err) = Product::save_many(products).await {
              let err = internal_error(action.name.clone())
                .with_message(format!("product_set: save_many error: {err:?}"));

              error!("product_set: ошибка записи Product.yaml: {err:?}");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            // отдаём обновлённый продукт
            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(product)), None) {
              info!("product_set: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("product_set: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== PRODUCT_DELETE =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("product_delete")
          {
            info!("product_delete: обработка запроса");

            if action.args.is_none() {
              let err =
                internal_error(action.name.clone()).with_message("product_delete: empty args");

              info!("product_delete: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_delete: to_replay_msg {}", err);
                });
              return Ok(());
            }

            let args = action.args.clone().unwrap();
            let id_opt: Option<Uuid> = match &args {
              serde_json::Value::String(s) => Uuid::parse_str(s.trim()).ok(),
              serde_json::Value::Object(obj) => obj
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s.trim()).ok()),
              _ => None,
            };

            let id = match id_opt {
              Some(id) => id,
              None => {
                let err = internal_error(action.name.clone())
                  .with_message("product_delete: bad args, expected {id:\"uuid\"} or \"uuid\"");

                info!("product_delete: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("product_delete: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            if id.is_nil() {
              let err =
                internal_error(action.name.clone()).with_message("product_delete: id is nil");

              info!("product_delete: шлём ошибку — пустой id");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_delete: to_replay_msg {}", err);
                });
              return Ok(());
            }

            info!("product_delete: входящий id = {}", id);

            let mut products = Product::load_list().await;

            let pos = match products.iter().position(|p| p.id() == &id) {
              Some(pos) => pos,
              None => {
                let err = internal_error(action.name.clone())
                  .with_message(format!("product_delete: product with id {id} not found"));

                error!("product_delete: продукт с id {id} не найден");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("product_delete: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let deleted = products.remove(pos);
            info!("product_delete: удалён продукт id={}", id);

            if let Err(err) = Product::save_many(products).await {
              let err = internal_error(action.name.clone())
                .with_message(format!("product_delete: save_many error: {err:?}"));

              error!("product_delete: ошибка записи Products.yaml: {err:?}");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("product_delete: to_replay_msg {}", err);
                });
              return Ok(());
            }
            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(deleted)), None) {
              info!("product_delete: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("product_delete: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== EVENT_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("event_list")
            && action.args.is_some()
          {
            info!("event_list: обработка запроса");

            let args = match serde_json::from_value::<EventListArgs>(action.args.clone().unwrap()) {
              Ok(args) => args,
              Err(err) => {
                let err =
                  internal_error(action.name.clone()).with_message(format!("event_list: {}", err));

                info!("event_list: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("event_list: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            info!(
              "event_list: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );

            let events = FacilityEvent::load_list().await;
            let rules: HashMap<_, _> = FacilityEventRule::load_list()
              .await
              .into_iter()
              .map(|v| (*v.id(), v))
              .collect();

            let data: Vec<FacilityEvent> = if args.ids.is_empty() {
              events
                .into_iter()
                .map(|v| {
                  let mut v = v;
                  v.rule = DataLink::Data(
                    rules
                      .iter()
                      .find(|r| r.1.id() == v.rule.id())
                      .unwrap()
                      .1
                      .clone(),
                  );
                  v
                })
                .collect()
            } else {
              let ids = args.ids;
              events
                .into_iter()
                .filter(|e| ids.contains(&e.id))
                .map(|v| {
                  let mut v = v;
                  v.rule = DataLink::Data(
                    rules
                      .iter()
                      .find(|r| r.1.id() == v.rule.id())
                      .unwrap()
                      .1
                      .clone(),
                  );
                  v
                })
                .collect()
            };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("event_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("event_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== DataChangeList =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("data_change_list")
            && action.args.is_some()
          {
            info!("data_change_list: обработка запроса");

            let args = match serde_json::from_value::<DataChangeArgs>(action.args.clone().unwrap())
            {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone())
                  .with_message(format!("data_change_list: {}", err));

                info!("data_change_list: шлём ошибку в ipc_router (bad args)");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("data_change_list: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let changes = DataChange::load_list().await;

            let data = if args.ids.as_ref().is_none() || args.ids.as_ref().unwrap().is_empty() {
              changes
            } else {
              let ids = args.ids.unwrap();
              changes
                .into_iter()
                .filter(|change| ids.contains(change.id()))
                .collect()
            };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("data_change_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("data_change_list: to_replay_msg вернул None");
            }

            return Ok(());
          }
          // ===================== EVENT_RULE_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("event_rule_list")
            && action.args.is_some()
          {
            let args =
              match serde_json::from_value::<EventRuleListArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone())
                    .with_message(format!("event_rule_list: {}", err));

                  info!("event_rule_list: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("event_rule_list: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            info!(
              "event_rule_list: входные аргументы: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );
            let events = FacilityEventRule::load_list().await;

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": events })), None) {
              info!("event_rule_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("event_rule_list: to_replay_msg вернул None");
            }

            return Ok(());
          }
          // ===================== EVENT_RULE_SET =====================
          if action.kind == IPCActionKind::SetData
            && (action.name.as_deref() == Some("event_rule_set")
              || action.name.as_deref() == Some("event_rule_create"))
          {
            info!("event_rule_set: обработка запроса");

            if action.args.is_none() {
              let err =
                internal_error(action.name.clone()).with_message("event_rule_set: empty args");

              info!("event_rule_set: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("event_rule_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            let rule: FacilityEventRule = if action.name.as_deref() == Some("event_rule_create") {
              let rule: EventRuleCreateArgs =
                match serde_json::from_value(action.args.clone().unwrap()) {
                  Ok(r) => r,
                  Err(err) => {
                    let err = internal_error(action.name.clone())
                      .with_message(format!("event_rule_create: {}", err));

                    info!("event_rule_create: шлём ошибку в ipc_router (bad args)");
                    let _ = state
                      .ipc_router
                      .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                      .map_err(|err| {
                        error!("event_rule_create: to_replay_msg {}", err);
                      });
                    return Ok(());
                  }
                };
              match rule.try_into() {
                Ok(r) => r,
                Err(err) => {
                  let err = internal_error(action.name.clone())
                    .with_message(format!("event_rule_create: {}", 0));

                  info!("event_rule_create: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("event_rule_create: to_replay_msg {}", 0);
                    });
                  return Ok(());
                }
              }
            } else {
              match serde_json::from_value(action.args.clone().unwrap()) {
                Ok(r) => r,
                Err(err) => {
                  let err = internal_error(action.name.clone())
                    .with_message(format!("event_rule_set: {}", err));

                  info!("event_rule_set: шлём ошибку в ipc_router (bad args)");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("event_rule_set: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              }
            };

            let rule_id = match &rule {
              FacilityEventRule::LimitsExceeded { id, .. } => *id,
              FacilityEventRule::HartStatus { id, .. } => *id,
            };

            let mut rules = FacilityEventRule::load_list().await;

            if let Some(pos) = rules.iter().position(|r| {
              let id = match r {
                FacilityEventRule::LimitsExceeded { id, .. } => id,
                FacilityEventRule::HartStatus { id, .. } => id,
              };
              *id == rule_id
            }) {
              info!("event_rule_set: обновляем существующее правило {}", rule_id);
              rules[pos] = rule.clone();
            } else {
              info!("event_rule_set: добавляем новое правило {}", rule_id);
              rules.push(rule.clone());
            }

            let _ = FacilityEventRule::save_many(rules).await;

            // В ответ отдаём само правило (Reply = FacilityEventRule)
            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!(rule)), None) {
              info!("event_rule_set: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("event_rule_set: to_replay_msg вернул None");
            }

            return Ok(());
          }
          // ===================== LOAD_GRAD_TABLE =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("load_grad_table")
          {
            info!("load_grad_table: обработка запроса");

            if action.args.is_none() {
              let err =
                internal_error(action.name.clone()).with_message("load_grad_table: empty args");

              info!("load_grad_table: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("load_grad_table: to_replay_msg {}", err);
                });
              return Ok(());
            }

            let args =
              match serde_json::from_value::<LoadGradTableArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone())
                    .with_message(format!("load_grad_table: bad args: {}", err));

                  error!("load_grad_table: bad args: {err:?}");
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("load_grad_table: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

            let device_id = args.device_id;
            info!("load_grad_table: device_id = {device_id}");

            let decoded = match general_purpose::STANDARD.decode(args.table.trim()) {
              Ok(bytes) => bytes,
              Err(err) => {
                let err = internal_error(action.name.clone())
                  .with_message(format!("load_grad_table: base64 decode error: {}", err));

                error!("load_grad_table: base64 decode error: {err:?}");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("load_grad_table: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let csv_str = match String::from_utf8(decoded) {
              Ok(s) => s,
              Err(err) => {
                let err = internal_error(action.name.clone())
                  .with_message(format!("load_grad_table: utf8 error: {}", err));

                error!("load_grad_table: utf8 error: {err:?}");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("load_grad_table: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            let mut grad_table: Vec<[f64; 3]> = Vec::new();

            for (line_no, line) in csv_str.lines().enumerate() {
              let line = line.trim();
              if line.is_empty() {
                continue;
              }

              if line_no == 0 {
                let first_token = line
                  .split([',', ';', '\t'])
                  .find(|t| !t.trim().is_empty())
                  .unwrap_or("");
                if first_token.parse::<f64>().is_err() {
                  info!("load_grad_table: skip header line: {}", line);
                  continue;
                }
              }

              let parts: Vec<_> = line
                .split([',', ';', '\t'])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

              if parts.len() < 3 {
                let err = internal_error(action.name.clone()).with_message(format!(
                  "load_grad_table: line {}: expected at least 3 columns, got {}",
                  line_no + 1,
                  parts.len()
                ));

                error!(
                  "load_grad_table: некорректное количество колонок в строке {}: {:?}",
                  line_no + 1,
                  parts
                );
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("load_grad_table: to_replay_msg {}", err);
                  });
                return Ok(());
              }

              let h: f64 = match parts[0].replace(',', ".").parse() {
                Ok(v) => v,
                Err(err) => {
                  let err = internal_error(action.name.clone()).with_message(format!(
                    "load_grad_table: line {}: bad level value '{}': {}",
                    line_no + 1,
                    parts[0],
                    err
                  ));
                  error!(
                    "load_grad_table: parse error line {} col 1: {} ({err:?})",
                    line_no + 1,
                    parts[0]
                  );
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("load_grad_table: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

              let v: f64 = match parts[1].replace(',', ".").parse() {
                Ok(v) => v,
                Err(err) => {
                  let err = internal_error(action.name.clone()).with_message(format!(
                    "load_grad_table: line {}: bad volume value '{}': {}",
                    line_no + 1,
                    parts[1],
                    err
                  ));
                  error!(
                    "load_grad_table: parse error line {} col 2: {} ({err:?})",
                    line_no + 1,
                    parts[1]
                  );
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("load_grad_table: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

              let dv: f64 = match parts[2].replace(',', ".").parse() {
                Ok(v) => v,
                Err(err) => {
                  let err = internal_error(action.name.clone()).with_message(format!(
                    "load_grad_table: line {}: bad dV value '{}': {}",
                    line_no + 1,
                    parts[2],
                    err
                  ));
                  error!(
                    "load_grad_table: parse error line {} col 3: {} ({err:?})",
                    line_no + 1,
                    parts[2]
                  );
                  let _ = state
                    .ipc_router
                    .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                    .map_err(|err| {
                      error!("load_grad_table: to_replay_msg {}", err);
                    });
                  return Ok(());
                }
              };

              grad_table.push([h, v, dv]);
            }

            info!(
              "load_grad_table: распарсили {} строк градуировочной таблицы",
              grad_table.len()
            );

            // --------- Сохраняем grad_table рядом с config.yaml ---------
            // Путь вида: assets/db/tanks/<device_id>/grad_table.json
            let tank_dir = format!("assets/db/tanks/{device_id}");
            let grad_path = format!("{tank_dir}/grad_table.json");

            info!(
              "load_grad_table: сохраняем grad_table в файл: {}",
              grad_path
            );

            // создаём директорию tanks/<device_id>, если её ещё нет
            if let Err(err) = fs::create_dir_all(&tank_dir) {
              let err = internal_error(action.name.clone()).with_message(format!(
                "load_grad_table: create_dir_all {tank_dir}: {err:?}"
              ));

              error!(
                "load_grad_table: не удалось создать директорию {}: {err:?}",
                tank_dir
              );
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("load_grad_table: to_replay_msg {}", err);
                });
              return Ok(());
            }

            // сериализуем grad_table как JSON (массив [h, v, dv])
            let grad_json = match serde_json::to_string_pretty(&grad_table) {
              Ok(s) => s,
              Err(err) => {
                let err = internal_error(action.name.clone()).with_message(format!(
                  "load_grad_table: serialize grad_table.json error: {}",
                  err
                ));

                error!(
                  "load_grad_table: ошибка сериализации grad_table для {}: {err:?}",
                  grad_path
                );
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("load_grad_table: to_replay_msg {}", err);
                  });
                return Ok(());
              }
            };

            // пишем файл grad_table.json
            if let Err(err) = fs::write(&grad_path, grad_json) {
              let err = internal_error(action.name.clone()).with_message(format!(
                "load_grad_table: write grad_table.json error: {err:?}"
              ));

              error!("load_grad_table: ошибка записи {}: {err:?}", grad_path);
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("load_grad_table: to_replay_msg {}", err);
                });
              return Ok(());
            }

            info!(
              "load_grad_table: успешно записали grad_table в {}",
              grad_path
            );
            info!("load_grad_table: sending success reply...");
            let msg = ipc_msg.to_replay_msg(Some(()), None);
            if let Err(e) = state.ipc_router.send_message(msg) {
              error!("load_grad_table: failed to send success reply: {e}");
            } else {
              info!("load_grad_table: success reply sent");
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

pub fn get_tank_full(id: Uuid) -> Result<Tank, anyhow::Error> {
  let args = serde_json::from_value::<TankListArgs>(
    serde_json::to_value(TankListArgs {
      fields: TankListFields::All,
      ids: vec![id],
    })
    .unwrap(),
  )?;

  let _path = "assets/db/tanks.yaml";
  let mut tanks: HashMap<_, _> = Tank::load_list_sync()
    .into_iter()
    .map(|v| (v.id, v))
    .collect();
  let ids: Vec<_> = if !args.ids.is_empty() {
    args.ids.to_vec()
  } else {
    tanks.keys().cloned().collect()
  };
  for id in ids.iter() {
    let tank = tanks.get_mut(id).unwrap();
    match args.fields {
      TankListFields::Minimal => {
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

  if !data.is_empty() {
    Ok(data[0].clone())
  } else {
    Err(anyhow::Error::msg("not_fond"))
  }
}
