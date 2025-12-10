use crate::actors::ipc_kmh_handler::KmhIpcHandlerMsg;
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;
use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::{BaseVars, ExtVars, Tank};
use crate::{
  EventListArgs, EventRuleListArgs, LoadGradTableArgs, ProductListArgs, TankListArgs,
  TankListFields,
};
use base64::Engine;
use base64::engine::general_purpose;
use chrono::Local;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::{json, to_string_pretty};
use std::collections::HashMap;
use std::fs::{self, File};
use taxon_core::actors::ipc::errors::internal_error;
use taxon_core::infrastructure::device::{FacilityEvent, FacilityEventRule};
use taxon_core::infrastructure::facility::{DataChange, DataLink, SharedData};
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

            let mut tanks: HashMap<_, _> = Tank::load_list("assets/db/")
              .await
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
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("tank_config_set")
          // && action.args.is_some()
          {
            let args = match serde_json::from_value::<TankConfig>(action.args.clone().unwrap()) {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone(), None)
                  .with_message(format!("tank_config_set: {}", err));

                error!("tank_config_set: {err:?}");
                let _ = state
                  .ipc_router
                  .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                  .map_err(|err| {
                    error!("tank_config_set: to_replay_msg {}", err);
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

            // let _change =
            //   DataChange::generate_changes(&old_config, &new_config, "admin".into(), Local::now());
            if let Err(err) = fs::write(
              config_vars_path,
              serde_saphyr::to_string(&new_config).unwrap(),
            ) {
              let err = internal_error(action.name.clone(), None)
                .with_message(format!("event_rule_set: write error: {err:?}"));

              error!("event_rule_set: ошибка записи eventrules.yaml: {err:?}");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("event_rule_set: to_replay_msg {}", err);
                });
              return Ok(());
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

            let tanks = Tank::load_list("assets/db/").await;

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

          // ===================== EVENT_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("event_list")
            && action.args.is_some()
          {
            info!("event_list: обработка запроса");

            let args = match serde_json::from_value::<EventListArgs>(action.args.clone().unwrap()) {
              Ok(args) => args,
              Err(err) => {
                let err = internal_error(action.name.clone(), None)
                  .with_message(format!("event_list: {}", err));

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

            let events = FacilityEvent::load_list("assets/db/").await;
            let rules: HashMap<_, _> = FacilityEventRule::load_list("assets/db/")
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

          // ===================== EVENT_RULE_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("event_rule_list")
            && action.args.is_some()
          {
            info!("event_rule_list: обработка запроса");

            let args =
              match serde_json::from_value::<EventRuleListArgs>(action.args.clone().unwrap()) {
                Ok(args) => args,
                Err(err) => {
                  let err = internal_error(action.name.clone(), None)
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
              "event_rule_list: ids.len() = {}, fields = {:?}",
              args.ids.len(),
              args.fields
            );

            let rules = FacilityEventRule::load_list("assets/db/").await;

            let data: Vec<FacilityEventRule> = if args.ids.is_empty() {
              rules
            } else {
              let ids = args.ids;
              rules
                .into_iter()
                .filter(|r| {
                  let rule_id = match r {
                    FacilityEventRule::LimitsExceeded { id, .. } => id,
                    FacilityEventRule::HartStatus { id, .. } => id,
                  };
                  ids.contains(rule_id)
                })
                .collect()
            };

            if let Some(msg) = ipc_msg.to_replay_msg(Some(json!({ "data": data })), None) {
              info!("event_rule_list: отправляем ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("event_rule_list: to_replay_msg вернул None");
            }

            return Ok(());
          }

          // ===================== EVENT_RULE_SET =====================
          if action.kind == IPCActionKind::SetData
            && action.name.as_deref() == Some("event_rule_set")
          {
            info!("event_rule_set: обработка запроса");

            if action.args.is_none() {
              let err = internal_error(action.name.clone(), None)
                .with_message("event_rule_set: empty args");

              info!("event_rule_set: шлём ошибку в ipc_router (empty args)");
              let _ = state
                .ipc_router
                .send_message(ipc_msg.to_replay_msg(Option::<()>::None, Some(err)))
                .map_err(|err| {
                  error!("event_rule_set: to_replay_msg {}", err);
                });
              return Ok(());
            }

            let rule: FacilityEventRule = match serde_json::from_value(action.args.clone().unwrap())
            {
              Ok(r) => r,
              Err(err) => {
                let err = internal_error(action.name.clone(), None)
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
            };

            let rule_id = match &rule {
              FacilityEventRule::LimitsExceeded { id, .. } => *id,
              FacilityEventRule::HartStatus { id, .. } => *id,
            };

            let mut rules = FacilityEventRule::load_list("assets/db/").await;

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

            let _ = FacilityEventRule::save_all(rules, "assets/db/").await;

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
              let err = internal_error(action.name.clone(), None)
                .with_message("load_grad_table: empty args");

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
                  let err = internal_error(action.name.clone(), None)
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
                let err = internal_error(action.name.clone(), None)
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
                let err = internal_error(action.name.clone(), None)
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
                let err = internal_error(action.name.clone(), None).with_message(format!(
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
                  let err = internal_error(action.name.clone(), None).with_message(format!(
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
                  let err = internal_error(action.name.clone(), None).with_message(format!(
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
                  let err = internal_error(action.name.clone(), None).with_message(format!(
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
              let err = internal_error(action.name.clone(), None).with_message(format!(
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
                let err = internal_error(action.name.clone(), None).with_message(format!(
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
              let err = internal_error(action.name.clone(), None).with_message(format!(
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
  let mut tanks: HashMap<_, _> = Tank::load_list_sync("assets/db")
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

  if !data.is_empty() {
    Ok(data[0].clone())
  } else {
    Err(anyhow::Error::msg("not_fond"))
  }
}
