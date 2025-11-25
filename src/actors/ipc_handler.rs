//Отправка и получение сообщений из ModBus_fabric в Ws
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use taxon_core::infrastructure::ExtModuleProtocol;
use taxon_core::prelude::{IPCActorMsg, IPCMessage, IPCMessageCrate, IPCMsgEncoding};
use tracing::{error, info};

use crate::actors::modbus_fabric_actor::ModbusFabricMsg;
use crate::actors::modbus_fabric_actor::ModbusDevice;

#[derive(Clone)]
pub struct Subscriber {
    pub peer: uuid::Uuid,
    pub protocol: ExtModuleProtocol,
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
    DevicesList(Vec<ModbusDevice>),
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
        info!("ikm_controller: received message");
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
                        }
                        Some("device_send") => {
                            // Можно сразу запросить данные у ModbusFabricActor
                            let _ = state.hart_fabric.send_message(ModbusFabricMsg::GetDevices(myself.clone()));
                        }
                        _ => {}
                    }
                }
            }

            // Когда ModbusFabricActor вернёт список устройств отправим в Ws
            IpcHandlerMsg::DevicesList(devices) => {
                info!("Отправка значений устройств подписчикам");
                //Тут сделать отправку значений подписчикам.

                if state.subscribers.is_empty() {
                info!("Нет подписчиков, отправка отменена");
                return Ok(());
                }
                
                let args: Vec<u16> = devices.iter().map(|d| d.value).collect();

                // Формируем ActionReply
                for sub in &state.subscribers { 
                    let reply = IPCMessage::ActionReply { 
                        id: 0, 
                        ok: Some(json!(args)), 
                        error: None, 
                        send_at: None, 
                    };

                // Отправляем через ipc_router в WS
                if let Err(e) = state.ipc_router.send_message(Some(IPCActorMsg::SendActionReply(
                    IPCMessageCrate {
                        msg: reply,
                        peer_from: uuid::Uuid::nil(), // Разобраться с uuid
                        encoding: IPCMsgEncoding::Json,
                        protocol: taxon_core::infrastructure::ExtModuleProtocol::WS,
                    },
                ))) {
                    error!("Ошибка при отправке списка устройств подписчику: {:?} {:?}",sub.peer, e);
                }
            }
            }
        }
                Ok(())
            }
        }
