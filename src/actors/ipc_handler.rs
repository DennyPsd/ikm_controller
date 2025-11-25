use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;
use taxon_core::prelude::{IPCActorMsg, IPCMessage, IPCMessageCrate, IPCMsgEncoding};
use tracing::{error, info};

use crate::actors::modbus_fabric_actor::ModbusFabricMsg;
use crate::actors::modbus_fabric_actor::ModbusDevice;

pub struct IpcHandler;

#[derive(Debug)]
pub struct IpcHandlerState {
    pub hart_fabric: ActorRef<ModbusFabricMsg>,
    #[allow(dead_code)]
    pub ipc_router: ActorRef<Option<IPCActorMsg>>,
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
    pub data: Option<SmolStr>, // base64 payload (optional)
}

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
                    if action.name.as_deref() == Some("device_send") {
                        // Запрашиваем список устройств у ModbusFabricActor
                        let _ = state.hart_fabric.send_message(ModbusFabricMsg::GetDevices(myself.clone()));
                    }
                }
            }

            // Когда ModbusFabricActor вернёт список устройств отправим в Ws
            IpcHandlerMsg::DevicesList(devices) => {
                let args: Vec<u16> = devices.iter().map(|d| d.value).collect();

                // Формируем ActionReply
                let reply = IPCMessage::ActionReply {
                    id: 0, // Можно взять id из исходного сообщения
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
                    error!("Ошибка при отправке списка устройств: {:?}", e);
                }
            }
        }
                Ok(())
            }
        }
    
