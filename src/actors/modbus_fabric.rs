// actors/modbus_fabric.rs
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use taxon_core::prelude::IPCProtocol;
use tokio_serial::SerialStream;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::actors::ipc_handler::IpcHandlerMsg;
use crate::actors::modbus_types::*;
use crate::actors::modbus_types::{ModbusSettings, ModbusTimings};
use crate::actors::modbus_worker::{ModbusWorker, ModbusWorkerMsg};
use crate::actors::modbus_worker_job::*;

// taxon types
use serde_json::json;
use smol_str::SmolStr as SS;
use taxon_core::infrastructure::device::{
    FacilityDevice, FacilityDeviceDocs, FacilityDeviceMeta, FacilityDeviceinfo, ModbusDeviceMeta,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
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
    GetDevicesOnce {
        // новое сообщение для одноразового запроса
        reply_to: ActorRef<IpcHandlerMsg>,
        peer: uuid::Uuid,
        protocol: IPCProtocol,
    },
    WriteDevice {
        device_idx: usize,
        value: u16,
    },
    PrintDevices,
    // New message from worker
    WorkerReport {
        port_name: SmolStr,
        slave: u16,
        addr: u16,
        raw: Option<f32>,
    },
}

pub struct ModbusFabricActor;

#[derive(Debug)]
pub struct ModbusFabricState {
    pub workers: HashMap<(SmolStr, String), ActorRef<ModbusWorkerMsg>>,
    pub devices: Vec<FacilityDevice>, // теперь FacilityDevice
    pub settings: ModbusSettings,
}

impl ModbusFabricActor {
    pub fn new(_devices: Vec<FacilityDevice>) -> Self {
        Self
    }
}

// --- helper: сохранение/загрузка списка устройств ---
fn devices_file_path() -> &'static str {
    "devices.json"
}

fn save_devices_to_file(devices: &Vec<FacilityDevice>) {
    match serde_json::to_string_pretty(devices) {
        Ok(txt) => {
            if let Err(e) = fs::write(devices_file_path(), txt) {
                error!("Failed to write devices file: {}", e);
            } else {
                info!("Devices saved to {}", devices_file_path());
            }
        }
        Err(e) => {
            error!("Failed to serialize devices: {}", e);
        }
    }
}

fn load_devices_from_file() -> Option<Vec<FacilityDevice>> {
    let path = Path::new(devices_file_path());
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(path) {
        Ok(txt) => match serde_json::from_str::<Vec<FacilityDevice>>(&txt) {
            Ok(devs) => Some(devs),
            Err(e) => {
                error!("Failed to parse devices file: {}", e);
                None
            }
        },
        Err(e) => {
            error!("Failed to read devices file: {}", e);
            None
        }
    }
}
// --------------------------------------------------

#[ractor::async_trait]
impl Actor for ModbusFabricActor {
    type Msg = ModbusFabricMsg;
    type State = ModbusFabricState;
    type Arguments = (Vec<FacilityDevice>, ModbusSettings);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (devices, settings): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Если есть файл — подгружаем, иначе используем args
        let devices = load_devices_from_file().unwrap_or(devices);
        info!(
            "ModbusFabric: starting with {} devices (in-memory)",
            devices.len()
        );
        Ok(ModbusFabricState {
            workers: HashMap::new(),
            devices,
            settings,
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
                info!(port = %port_name, "ModBusFabric: AttachPort called");

                // Check if this port matches the configured com_port
                if port_name != state.settings.modbus.com_port {
                    info!(port = %port_name, "ModBusFabric: port does not match configured com_port, ignoring");
                    return Ok(());
                }

                // Stop and remove old worker for this port
                if let Some(old_worker) = state.workers.remove(&(port_name.clone(), "".to_string()))
                {
                    let _ = old_worker.cast(ModbusWorkerMsg::Stop);
                }
                state.devices.retain(|d| d.port_address != port_name);

                let timings = ModbusTimings {
                    first_byte_timeout: std::time::Duration::from_millis(200),
                    per_byte_timeout: std::time::Duration::from_millis(50),
                    max_preamble_ff: 0,
                };

                // Spawn one worker for all sensors
                let (worker_ref, _jh) = ractor::Actor::spawn(
                    Some(format!("modbus_worker:{}", port_name)),
                    ModbusWorker::new(),
                    (
                        timings,
                        stream,
                        myself.clone(),
                        port_name.clone(),
                        state.settings.sensors.clone(),
                        state.settings.modbus.polling_ms,
                    ),
                )
                .await
                .map_err(|e| ActorProcessingErr::from(e.to_string()))?;

                state
                    .workers
                    .insert((port_name.clone(), "".to_string()), worker_ref);
                info!(port = %port_name, "ModBusFabric: worker spawned");

                // Create FacilityDevice for each sensor
                for sensor in &state.settings.sensors {
                    let mut attrs = BTreeMap::new();
                    attrs.insert(SS::from("value"), json!(0.0));
                    attrs.insert(SS::from("mul"), json!(1.0));
                    attrs.insert(SS::from("value_type"), json!("f32"));

                    let device = FacilityDevice {
                        device_id: Uuid::new_v4(),
                        device_type: sensor.name.clone().into(),
                        port_address: port_name.to_string().into(),
                        meta: FacilityDeviceMeta::Modbus {
                            data: ModbusDeviceMeta {
                                slave: sensor.slave as u16,
                                addr: sensor.start_reg,
                                reg: sensor.reg_type as u8,
                            },
                        },
                        connected: true,
                        attrs: Some(attrs),
                        info: None,
                        docs: None,
                        events: None,
                        active_events: [0; 8],
                        diagnostic: None,
                    };
                    state.devices.push(device);
                }

                // сохраняем список в файл
                save_devices_to_file(&state.devices);
            }

            ModbusFabricMsg::DetachPort { port_name } => {
                info!(port = %port_name, "ModBusFabric: DetachPort");
                if let Some(wr) = state.workers.remove(&(port_name.clone(), "".to_string())) {
                    let _ = wr.cast(ModbusWorkerMsg::Stop);
                    // remove devices from this port
                    state.devices.retain(|d| d.port_address != port_name);
                    info!(port = %port_name, "ModBusFabric: devices removed for port");
                    // сохраняем изменения
                    save_devices_to_file(&state.devices);
                }
            }

            ModbusFabricMsg::GetDevices(reply_to) => {
                let devices_clone = state.devices.clone();
                let _ = reply_to.send_message(
                    crate::actors::ipc_handler::IpcHandlerMsg::DevicesList(devices_clone),
                );
            }

            ModbusFabricMsg::GetDevicesOnce {
                reply_to,
                peer,
                protocol,
            } => {
                let devices_clone = state.devices.clone();
                let _ = reply_to.send_message(
                    crate::actors::ipc_handler::IpcHandlerMsg::DevicesListOnce {
                        devices: devices_clone,
                        peer: peer.clone(),
                        protocol: protocol.clone(),
                    },
                );
                println!("{} {:?}", peer, protocol);
            }

            ModbusFabricMsg::WriteDevice { device_idx, value } => {
                // For compatibility: update raw attrs.value if exists
                if let Some(dev) = state.devices.get_mut(device_idx) {
                    if let Some(attrs) = dev.attrs.as_mut() {
                        attrs.insert(SS::from("value"), json!(value));
                    }
                    info!(
                        "ModbusFabric: device idx {} updated = {}",
                        device_idx, value
                    );
                    //сохраняем
                    save_devices_to_file(&state.devices);
                } else {
                    warn!(
                        "ModbusFabric: WriteDevice: index {} out of range",
                        device_idx
                    );
                }
            }

            ModbusFabricMsg::PrintDevices => {
                info!("--- Devices list ---");
                for (i, d) in state.devices.iter().enumerate() {
                    let value = d
                        .attrs
                        .as_ref()
                        .and_then(|m| m.get(&SS::from("value")))
                        .cloned();
                    info!(idx = i, port = %d.port_address, meta = ?d.meta, value = ?value, "device");
                }
            }

            ModbusFabricMsg::WorkerReport {
                port_name,
                slave,
                addr,
                raw,
            } => {
                // Update devices that match port + slave + addr (if meta matches)
                let mut updated = 0usize;
                for dev in state.devices.iter_mut() {
                    // match port address
                    if dev.port_address != port_name.to_string() {
                        continue;
                    }

                    // match modbus meta
                    match &dev.meta {
                        FacilityDeviceMeta::Modbus { data } => {
                            if data.slave as u16 == slave && data.addr as u16 == addr {
                                // compute final value with mul if present
                                let raw_value = raw.unwrap_or(0.0);
                                let mul = dev
                                    .attrs
                                    .as_ref()
                                    .and_then(|m| m.get(&SS::from("mul")))
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(1.0);
                                let final_value = serde_json::Value::from((raw_value as f64) * mul);

                                if let Some(attrs) = dev.attrs.as_mut() {
                                    attrs.insert(SS::from("value"), final_value);
                                } else {
                                    let mut map = BTreeMap::new();
                                    map.insert(SS::from("value"), json!(final_value));
                                    dev.attrs = Some(map);
                                }
                                dev.connected = raw.is_some();
                                updated += 1;
                            }
                        }
                        _ => {}
                    }
                }
                if updated > 0 {
                    info!(port=%port_name, matched = updated, "ModbusFabric: updated device values from worker");
                    // сохраняем изменения в файл
                    //save_devices_to_file(&state.devices);
                } else {
                    info!(port=%port_name, "ModBusFabric: WorkerReport received but no matching device found");
                }
            }
        }

        Ok(())
    }
}
