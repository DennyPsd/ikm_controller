//Отправка и получение сообщений из ModBus_fabric в Ws
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use taxon_core::infrastructure::device::FacilityDevice;
use taxon_core::prelude::IPCProtocol;
use taxon_core::prelude::{IPCActorMsg, IPCMessage, IPCMessageCrate, IPCMsgEncoding};
use tracing::{error, info};

use crate::actors::modbus_fabric::ModbusFabricMsg;
use crate::actors::modbus_fabric::ModbusDevice;

#[derive(Clone)]
pub struct Subscriber {
    pub peer: uuid::Uuid,
    pub protocol: IPCProtocol,
}

//Позже переименовать статус hart в modbus
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
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        info!("ipc_handler started");
        Ok(args)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        info!("ipc_handler: received message");
        match msg {
            IpcHandlerMsg::Ipc(ipc_msg) => {
                println!("*Обработка сообщения по IPC*");
                
                                // Проверяем, что это Action с командой device_send
                if let Some(action) = ipc_msg.as_action() {
                    match action.name.as_deref() {
                        Some("subscribe to vars") => {
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
                            // Можно сразу запросить данные у ModbusFabricActor
                            let _ = state.hart_fabric.send_message(ModbusFabricMsg::GetDevices(myself.clone()));
                        }
                        _ => {}
                    }
                }
            }

            IpcHandlerMsg::Tick => {
                // Запрашиваем актуальные значения у Fabric
                if state.subscribers.is_empty() {
                    info!("Нет подписчиков, отправка отменена");
                    return Ok(());
                }
                let _ = state.hart_fabric.send_message(ModbusFabricMsg::GetDevices(myself.clone()));
                // Перезапускаем таймер через 2 секунды
                let _ = myself.send_after(std::time::Duration::from_secs(2), || IpcHandlerMsg::Tick);
            }


            // Когда ModbusFabricActor вернёт список устройств отправим в Ws
            IpcHandlerMsg::DevicesList(devices) => {
                info!("Отправка значений устройств подписчикам и по запросу");
                //Тут сделать отправку значений подписчикам.

                // if state.subscribers.is_empty() {
                //     info!("Нет подписчиков, отправка отменена");
                //     return Ok(());
                // }
                if devices.is_empty() {
                    return Ok(());
                }

                // собираем массив значений
                    let values: Vec<serde_json::Value> = devices
                    .iter()
                    .map(|d| d.attrs.as_ref().and_then(|m| m.get(&SmolStr::from("value"))).cloned().unwrap_or(json!(0)))
                    .collect();

                // Формируем ActionReply для каждого подписчика
                for sub in &state.subscribers {
                    let reply = IPCMessage::ActionReply {
                        id: 0,
                        ok: Some(json!(values)),
                        error: None,
                        send_at: None,
                    };

                    // Отправляем через ipc_router — указываем peer_from равным подписчику
                    if let Err(e) = state.ipc_router.send_message(Some(IPCActorMsg::Send(
                        IPCMessageCrate {
                            msg: reply,
                            peer_from: sub.peer,
                            encoding: IPCMsgEncoding::Json,
                            protocol: sub.protocol.clone(),
                        },
                    ))) {
                        error!("Ошибка при отправке списка устройств подписчику {:?}: {:?}", sub.peer, e);
                    }
                }
            }
        }
        Ok(())
    }
}
