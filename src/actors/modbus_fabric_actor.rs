// Работа со списком устройств ModBus (вывод, изменение)
// TODO: Сделать запрос в calc-модуль. Только хз какой calc будет
// modbus_fabric.rs
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use tokio_serial::SerialStream;
use tracing::{info, warn};

use crate::actors::ipc_handler::IpcHandlerMsg;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusDevice {
    pub port: String,
    pub slave: u8,
    pub addres: u16,
    pub value: u16,
}

#[derive(Debug)]
pub enum ModbusFabricMsg {
    /// Register port and (later) spawn worker
    AttachPort {
        port_name: SmolStr,
        stream: SerialStream,
    },
    /// Detach port, stop worker
    DetachPort {
        port_name: SmolStr,
    },
    /// Simple: return devices list to requester actor
    GetDevices(ActorRef<IpcHandlerMsg>),

    /// Optional: write device value (index in devices vec)
    WriteDevice { device_idx: usize, value: u16 },

    /// Debug print
    PrintDevices,
}

pub struct ModbusFabricActor;

#[derive(Default)]
pub struct ModbusFabricState {
    /// map port -> "worker present" (we don't implement worker actor here, only store stream info if needed)
    pub workers: HashMap<SmolStr, ()>,
    /// devices list (loaded from settings.yaml)
    pub devices: Vec<ModbusDevice>,
    // map port -> stream (kept optional, for later use)
    // pub streams: HashMap<SmolStr, SerialStream>, // can't store SerialStream if not cloneable; store metadata if needed
}

impl ModbusFabricActor {
    pub fn new(devices: Vec<ModbusDevice>) -> Self {
        Self
    }
}

#[ractor::async_trait]
impl Actor for ModbusFabricActor {
    type Msg = ModbusFabricMsg;
    type State = ModbusFabricState;
    type Arguments = Vec<ModbusDevice>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(ModbusFabricState {
            workers: HashMap::new(),
            devices: args,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusFabricMsg::AttachPort { port_name, stream } => {
                // Пока просто логируем порт и stream (как просили)
                info!(port = %port_name, stream = ?stream, "ModbusFabric: AttachPort called");

                // Запомним, что воркер для этого порта есть (пока без реального воркера)
                state.workers.insert(port_name.clone(), ());
                // (Опционально) тут можно сохранить stream в state.streams если понадобится
            }

            ModbusFabricMsg::DetachPort { port_name } => {
                info!(port = %port_name, "ModbusFabric: DetachPort called");
                state.workers.remove(&port_name);
            }

            ModbusFabricMsg::GetDevices(reply_to) => {
                // Отвечаем отправителю полным списком устройств (клонируем)
                let devices_clone = state.devices.clone();
                // Отправляем сообщение IpcHandlerMsg::DevicesList
                let _ = reply_to.send_message(crate::actors::ipc_handler::IpcHandlerMsg::DevicesList(devices_clone));
            }

            ModbusFabricMsg::WriteDevice { device_idx, value } => {
                if let Some(dev) = state.devices.get_mut(device_idx) {
                    dev.value = value;
                    info!("ModbusFabric: device idx {} updated = {}", device_idx, value);
                } else {
                    warn!("ModbusFabric: WriteDevice: index {} out of range", device_idx);
                }
            }

            ModbusFabricMsg::PrintDevices => {
                info!("--- Devices list ---");
                for (i, d) in state.devices.iter().enumerate() {
                    info!(idx = i, port = %d.port, slave = d.slave, addr = d.addres, value = d.value, "device");
                }
            }
        }

        Ok(())
    }
}
