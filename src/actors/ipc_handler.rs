use std::collections::HashSet;
use std::fs;

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::{Value as JsonValue, json, to_string_pretty};
use serde_yaml::Value as YamlValue;
use smol_str::SmolStr;
use taxon_core::actors::ipc::errors::IPCError;
use taxon_core::prelude::{IPCActionKind, IPCActorMsg, IPCMessageCrate};
use tracing::{error, info};

use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;

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
          info!(
            "Modbus IPC_Handler: action.name={:?}, kind={:?}, target.module_name={:?}, data_ns={:?}",
            action.name, action.kind, action.target.module_name, action.target.data_ns,
          );

          // ===================== TANK_LIST =====================
          if action.kind == IPCActionKind::GetData && action.name.as_deref() == Some("tank_list") {
            info!("Modbus IPC_Handler: обработка action 'tank_list'");

            let full_info = action
              .args
              .as_ref()
              .and_then(|v| v.get("full"))
              .and_then(|v| v.as_bool())
              .unwrap_or(false);

            info!("tank_list: full_info={}", full_info);

            // ids фильтр (по id танка, строка)
            let ids: Vec<String> = action
              .args
              .as_ref()
              .and_then(|v| v.get("ids"))
              .and_then(|v| v.as_array())
              .map(|arr| {
                arr
                  .iter()
                  .filter_map(|x| x.as_str().map(|s| s.to_string()))
                  .collect()
              })
              .unwrap_or_default();

            info!("tank_list: ids filter = {:?}", ids);

            // park_ids на будущее
            let park_ids: Vec<SmolStr> = action
              .args
              .as_ref()
              .and_then(|v| v.get("park_ids"))
              .and_then(|v| v.as_array())
              .map(|arr| {
                arr
                  .iter()
                  .filter_map(|x| x.as_str().map(SmolStr::from))
                  .collect()
              })
              .unwrap_or_default();

            if !park_ids.is_empty() {
              info!("tank_list: park_ids filter = {:?}", park_ids);
            }

            let path = "assets/db/tanks.yaml";
            info!("tank_list: читаем файл '{}'", path);

            let yaml_str = match fs::read_to_string(path) {
              Ok(s) => {
                info!(
                  "tank_list: файл '{}' прочитан, длина={} байт",
                  path,
                  s.len()
                );
                s
              }
              Err(e) => {
                error!("tank_list: failed to read {}: {:?}", path, e);

                // важно: IPCError<()> и args=None
                let err: IPCError<()> = IPCError {
                  code: 500,
                  message: format!("tank_list: не удалось прочитать {}", path).into(),
                  action_name: action.name.clone(),
                  target: None,
                  args: None,
                };

                if let Some(msg) = ipc_msg.to_replay_msg::<JsonValue>(None, Some(err)) {
                  info!("tank_list: шлём ошибку в ipc_router (read_to_string)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("tank_list: to_replay_msg вернул None при ошибке чтения файла");
                }
                return Ok(());
              }
            };

            let mut tanks: Vec<YamlValue> = match serde_yaml::from_str::<Vec<YamlValue>>(&yaml_str)
            {
              Ok(v) => {
                info!("tank_list: YAML распарсен, элементов={}", v.len());
                v
              }
              Err(e) => {
                error!("tank_list: failed to parse YAML {}: {:?}", path, e);

                let err: IPCError<()> = IPCError {
                  code: 500,
                  message: "tank_list: ошибка парсинга tanks.yaml".into(),
                  action_name: action.name.clone(),
                  target: None,
                  args: None,
                };

                if let Some(msg) = ipc_msg.to_replay_msg::<JsonValue>(None, Some(err)) {
                  info!("tank_list: шлём ошибку в ipc_router (parse_yaml)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("tank_list: to_replay_msg вернул None при ошибке парсинга YAML");
                }
                return Ok(());
              }
            };

            if !ids.is_empty() {
              let before = tanks.len();
              tanks.retain(|t| {
                t.get("id")
                  .and_then(|v| v.as_str())
                  .map(|s| ids.iter().any(|id| id == s))
                  .unwrap_or(false)
              });
              let after = tanks.len();
              info!("tank_list: фильтр по ids: до={} после={}", before, after);
            }

            if !full_info {
              info!(
                "tank_list: full=false, отдаём {} танков как есть",
                tanks.len()
              );

              let reply_body = json!({
                "data": tanks,
              });

              if let Some(msg) = ipc_msg.to_replay_msg(Some(reply_body), None) {
                info!("tank_list: отправляем ответ в ipc_router (minimal)");
                let _ = state.ipc_router.send_message(Some(msg));
              } else {
                error!("tank_list: to_replay_msg вернул None при full=false");
              }

              return Ok(());
            }

            info!("tank_list: full=true, обогащаем данными из assets/db/calib/{{id}}/info.json");

            let mut enriched: Vec<JsonValue> = Vec::new();

            for t in tanks {
              let id_opt = t.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());

              if let Some(id) = id_opt {
                let calib_path = format!("assets/db/calib/{}/info.json", id);
                info!("tank_list: пытаемся прочитать '{}'", calib_path);

                if let Ok(info_str) = fs::read_to_string(&calib_path) {
                  info!(
                    "tank_list: calib-файл '{}' прочитан, длина={} байт",
                    calib_path,
                    info_str.len()
                  );
                  match serde_json::from_str::<JsonValue>(&info_str) {
                    Ok(meta) => {
                      info!("tank_list: calib json для '{}' успешно распарсен", id);
                      let obj = json!({
                        "tank": t,
                        "meta": meta,
                      });
                      enriched.push(obj);
                    }
                    Err(e) => {
                      error!(
                        "tank_list: parse calib json failed for {}: {:?}",
                        calib_path, e
                      );
                      enriched.push(json!({ "tank": t }));
                    }
                  }
                } else {
                  info!(
                    "tank_list: calib-файл '{}' не найден или не читается, кладём только tank",
                    calib_path
                  );
                  enriched.push(json!({ "tank": t }));
                }
              } else {
                error!("tank_list: у элемента нет поля 'id', кладём как есть");
                enriched.push(json!({ "tank": t }));
              }
            }

            info!("tank_list: enriched.len()={}", enriched.len());

            let reply_body = json!({
              "data": enriched,
            });

            if let Some(msg) = ipc_msg.to_replay_msg(Some(reply_body), None) {
              info!("tank_list: отправляем full-ответ в ipc_router");
              let _ = state.ipc_router.send_message(Some(msg));
            } else {
              error!("tank_list: to_replay_msg вернул None при full=true");
            }

            return Ok(());
          }

          // ===================== PARK_LIST =====================
          if action.kind == IPCActionKind::GetData && action.name.as_deref() == Some("park_list") {
            info!("Modbus IPC_Handler: обработка action 'park_list'");
            return Ok(());
          }

          // ===================== PRODUCTS_LIST =====================
          if action.kind == IPCActionKind::GetData
            && action.name.as_deref() == Some("products_list")
          {
            info!("Modbus IPC_Handler: обработка action 'products_list'");

            let full_info = action
              .args
              .as_ref()
              .and_then(|v| v.get("full"))
              .and_then(|v| v.as_bool())
              .unwrap_or(false);

            let ids: Vec<String> = action
              .args
              .as_ref()
              .and_then(|v| v.get("ids"))
              .and_then(|v| v.as_array())
              .map(|arr| {
                arr
                  .iter()
                  .filter_map(|x| x.as_str().map(|s| s.to_string()))
                  .collect()
              })
              .unwrap_or_default();

            info!(
              "products_list: full_info={}, ids_filter={:?}",
              full_info, ids
            );

            let path = "assets/db/tanks.yaml";
            info!("products_list: читаем файл '{}'", path);

            let yaml_str = match fs::read_to_string(path) {
              Ok(s) => {
                info!(
                  "products_list: файл '{}' прочитан, длина={} байт",
                  path,
                  s.len()
                );
                s
              }
              Err(e) => {
                error!("products_list: failed to read {}: {:?}", path, e);

                let err: IPCError<()> = IPCError {
                  code: 500,
                  message: format!("products_list: не удалось прочитать {}", path).into(),
                  action_name: action.name.clone(),
                  target: None,
                  args: None,
                };

                if let Some(msg) = ipc_msg.to_replay_msg::<JsonValue>(None, Some(err)) {
                  info!("products_list: шлём ошибку в ipc_router (read_to_string)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("products_list: to_replay_msg вернул None при ошибке чтения");
                }
                return Ok(());
              }
            };

            let tanks: Vec<YamlValue> = match serde_yaml::from_str::<Vec<YamlValue>>(&yaml_str) {
              Ok(v) => {
                info!("products_list: tanks YAML распарсен, элементов={}", v.len());
                v
              }
              Err(e) => {
                error!("products_list: failed to parse YAML {}: {:?}", path, e);

                let err: IPCError<()> = IPCError {
                  code: 500,
                  message: "products_list: ошибка парсинга tanks.yaml".into(),
                  action_name: action.name.clone(),
                  target: None,
                  args: None,
                };

                if let Some(msg) = ipc_msg.to_replay_msg::<JsonValue>(None, Some(err)) {
                  info!("products_list: шлём ошибку в ipc_router (parse_yaml)");
                  let _ = state.ipc_router.send_message(Some(msg));
                } else {
                  error!("products_list: to_replay_msg вернул None при ошибке парсинга YAML");
                }
                return Ok(());
              }
            };

            let mut unique_products: Vec<JsonValue> = Vec::new();
            let mut seen_keys: HashSet<String> = HashSet::new();

            for t in &tanks {
              let product_yaml = match t.get("product") {
                Some(p) => p,
                None => continue, // в этом танке продукт не описан
              };

              let prod_json: JsonValue =
                match serde_yaml::from_value::<JsonValue>(product_yaml.clone()) {
                  Ok(v) => v,
                  Err(e) => {
                    error!(
                      "products_list: не смог распарсить product из tanks.yaml: {:?}",
                      e
                    );
                    continue;
                  }
                };

              let key = prod_json
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                  prod_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                })
                .unwrap_or_else(|| serde_json::to_string(&prod_json).unwrap_or_default());

              if !ids.is_empty() {
                let match_by_id = prod_json
                  .get("id")
                  .and_then(|v| v.as_str())
                  .map(|s| ids.iter().any(|id| id == s))
                  .unwrap_or(false);

                let match_by_key = ids.iter().any(|id| id == &key);

                if !(match_by_id || match_by_key) {
                  continue;
                }
              }

              if seen_keys.insert(key) {
                unique_products.push(prod_json);
              }
            }

            info!(
              "products_list: найдено {} уникальных продуктов",
              unique_products.len()
            );

            let reply_body = json!({
              "data": unique_products,
            });

            if let Some(msg) = ipc_msg.to_replay_msg(Some(reply_body), None) {
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
        } else {
          info!("Modbus IPC_Handler: ipc_msg.as_action() = None, сообщение не Action — игнорируем");
        }

        Ok(())
      }
    }
  }
}
