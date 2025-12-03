// Получает запросы от WebSocket IPC, обрабатывает подписки и отправляет данные подписчикам.
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use std::time::Duration;
use tracing::{error, info};

use taxon_core::infrastructure::device::FacilityDevice;
use taxon_core::prelude::{IPCActorMsg, IPCMessage, IPCMessageCrate, IPCMsgEncoding, IPCProtocol};

use crate::actors::modbus_fabric::ModbusFabricMsg;

#[derive(Clone)]
pub struct Subscriber {
    pub peer: uuid::Uuid,
    pub protocol: IPCProtocol,
}

pub struct IpcHandlerState {
    pub hart_fabric: ActorRef<ModbusFabricMsg>,
    #[allow(dead_code)]
    pub ipc_router: ActorRef<Option<IPCActorMsg>>,
    pub subscribers: Vec<Subscriber>,
}

#[derive(Debug)]
pub enum IpcHandlerMsg {
    Ipc(IPCMessageCrate),
    DevicesList(Vec<FacilityDevice>),
    Tick,
    DevicesListOnce {
        devices: Vec<FacilityDevice>,
        peer: uuid::Uuid,
        protocol: IPCProtocol,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SendToDeviceArgs {
    #[serde(default)]
    pub command: u8,
    #[serde(default)]
    pub data: Option<SmolStr>,
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
        info!("IPC_Handler запущен");
        Ok(args)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        info!("IPC_Handler: получено сообщение");

        match msg {
            IpcHandlerMsg::Ipc(ipc_msg) => {
                println!("*Обработка сообщения по IPC*");

                if let Some(action) = ipc_msg.as_action() {
                    match action.name.as_deref() {
                        Some("subscribe_to_vars") => {
                            info!("Добавляем подписчика на переменные");
                            state.subscribers.push(Subscriber {
                                peer: ipc_msg.peer_from,
                                protocol: ipc_msg.protocol,
                            });

                            // Если это первый подписчик, запускаем Tick
                            if state.subscribers.len() == 1 {
                                let _ = myself.cast(IpcHandlerMsg::Tick);
                            }
                        }

                        Some("device_send") => {
                            // Всегда отправляем список устройств разово, даже если подписчиков нет
                            let reply_to = myself.clone();
                            let peer = ipc_msg.peer_from;
                            let protocol = ipc_msg.protocol.clone();

                            let _ =
                                state
                                    .hart_fabric
                                    .send_message(ModbusFabricMsg::GetDevicesOnce {
                                        reply_to,
                                        peer,
                                        protocol,
                                    });
                        }
                        Some("unsubscribe_to_vars") => {
                            info!("Удаляем подписчика на переменные");
                            state.subscribers.retain(|sub| {
                                (sub.peer == ipc_msg.peer_from && sub.protocol == ipc_msg.protocol)
                            });
                        }

                        _ => {}
                    }
                }
            }

            IpcHandlerMsg::Tick => {
                // Если нет подписчиков — не планируем Tick
                if state.subscribers.is_empty() {
                    info!("Нет подписчиков, Tick отменён");
                    return Ok(());
                }

                // Запрашиваем актуальные значения у Fabric
                let _ = state
                    .hart_fabric
                    .send_message(ModbusFabricMsg::GetDevices(myself.clone()));
            }

            IpcHandlerMsg::DevicesList(devices) => {
                info!("Отправка значений устройств подписчикам");

                // Если подписчиков нет — не планируем новый Tick
                if state.subscribers.is_empty() {
                    info!("Нет подписчиков, DevicesList обработан, Tick не планируется");
                    return Ok(());
                }

                // Если устройств нет — просто возвращаемся
                if devices.is_empty() {
                    info!("Список устройств пуст — Tick приостановлен");
                    return Ok(());
                }

                // собираем массив значений
                let values: Vec<serde_json::Value> = devices
                    .iter()
                    .map(|d| {
                        d.attrs
                            .as_ref()
                            .and_then(|m| m.get(&SmolStr::from("value")))
                            .cloned()
                            .unwrap_or(json!(0))
                    })
                    .collect();

                // Формируем ActionReply для каждого подписчика
                for sub in &state.subscribers {
                    let reply = IPCMessage::ActionReply {
                        id: 0,
                        ok: Some(json!(values)),
                        error: None,
                        send_at: None,
                    };

                    if let Err(e) =
                        state
                            .ipc_router
                            .send_message(Some(IPCActorMsg::Send(IPCMessageCrate {
                                msg: reply,
                                peer_from: sub.peer,
                                encoding: IPCMsgEncoding::Json,
                                protocol: sub.protocol.clone(),
                            })))
                    {
                        error!(
                            "Ошибка при отправке списка устройств подписчику {:?}: {:?}",
                            sub.peer, e
                        );
                    }
                }

                // Планируем следующий Tick через 2 секунды только если есть подписчики
                let _ = myself.send_after(Duration::from_secs(2), || IpcHandlerMsg::Tick);
            }

            // Разовое сообщение в WS при запросе без подписки
            IpcHandlerMsg::DevicesListOnce {
                devices,
                peer,
                protocol,
            } => {
                // собираем массив значений
                let values: Vec<serde_json::Value> = devices
                    .iter()
                    .map(|d| {
                        d.attrs
                            .as_ref()
                            .and_then(|m| m.get(&SmolStr::from("value")))
                            .cloned()
                            .unwrap_or(json!(0))
                    })
                    .collect();

                // Формируем ActionReply
                let reply = crate::actors::ipc_handler::IPCMessage::ActionReply {
                    id: 0,
                    ok: Some(json!(values)),
                    error: None,
                    send_at: None,
                };

                if let Err(e) =
                    state
                        .ipc_router
                        .send_message(Some(IPCActorMsg::Send(IPCMessageCrate {
                            msg: reply,
                            peer_from: peer,
                            encoding: IPCMsgEncoding::Json,
                            protocol,
                        })))
                {
                    error!("Ошибка при отправке списка устройств {:?}: {:?}", peer, e);
                }
            }
        }

        Ok(())
    }
}
