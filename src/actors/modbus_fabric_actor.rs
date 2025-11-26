// modbus_fabric_actor.rs
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use tokio_serial::SerialStream;
use tracing::{info, warn};

use crate::actors::modbus_types::ModbusTimings;
use crate::actors::modbus_worker::{ModbusWorker, ModbusWorkerMsg};
use crate::actors::modbus_types::ModbusBusConfig;

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
    AttachPort {
        port_name: SmolStr,
        stream: SerialStream,
    },
    DetachPort {
        port_name: SmolStr,
    },
    GetDevices(ActorRef<IpcHandlerMsg>),
    WriteDevice { device_idx: usize, value: u16 },
    PrintDevices,
}

pub struct ModbusFabricActor;

#[derive(Default)]
pub struct ModbusFabricState {
    pub workers: HashMap<SmolStr, ActorRef<ModbusWorkerMsg>>,
    pub devices: Vec<ModbusDevice>,
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
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusFabricMsg::AttachPort { port_name, stream } => {
                info!(port = %port_name, "ModbusFabric: Вызвано подключение порта");

                if state.workers.contains_key(&port_name) {
                    info!(port = %port_name, "ModbusFabric: воркер уже есть, пропускаем");
                } else {
                    // create timings default
                    let timings = ModbusTimings {
                        first_byte_timeout: std::time::Duration::from_millis(200),
                        per_byte_timeout: std::time::Duration::from_millis(50),
                        max_preamble_ff: 0,
                    };

                    // spawn worker actor (linked)
                    let (worker_ref, _jh) = Actor::spawn_linked(
                        Some(format!("modbus-worker:{}", port_name)),
                        ModbusWorker::new(),
                        (timings, stream),
                        myself.get_cell(),
                    )
                    .await
                    .map_err(|e| ActorProcessingErr::from(e.to_string()))?;

                    state.workers.insert(port_name.clone(), worker_ref);
                    info!(port = %port_name, "ModbusFabric: worker spawned");
                }
            }

            ModbusFabricMsg::DetachPort { port_name } => {
                info!(port = %port_name, "ModbusFabric: Вызвано отключение порта");
                if let Some(wr) = state.workers.remove(&port_name) {
                    let _ = wr.cast(ModbusWorkerMsg::Stop);
                }
            }

            ModbusFabricMsg::GetDevices(reply_to) => {
                let devices_clone = state.devices.clone();
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
