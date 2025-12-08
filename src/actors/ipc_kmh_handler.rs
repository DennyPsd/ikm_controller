use std::fs::File;
use std::path::Path;

use crate::KMHReportListArgs;
use crate::types::kmh::KMHReportInstance;
use crate::types::tanks::Tank;
use ikm_calc::calculation::kmh::KMHCalculator;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;
use taxon_core::actors::ipc::errors::internal_error;
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
          // ===================== KMH_LIST =====================
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

            // Собираем список tank_ids
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
            info!("KmhIpcHandler: обработка action 'kmh_set'");

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
              kmh_instance.device_id, kmh_instance.id
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

          // Если не наши kmh-экшены — просто игнор
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
