use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use tokio_serial::SerialStream;
use tracing::{info, warn};

use crate::actors::modbus_types::ModbusTimings;
use crate::actors::modbus_worker::{ModbusWorker, ModbusWorkerMsg, ModbusReplyBatchMsg};
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
    Tick,
    ReadResult { device_idx: usize, value: u16 },
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
        // стартуем тик, который можно затем перезапустить из IpcHandler
        let _ = _myself_placeholder();
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
                info!(port = %port_name, "ModbusFabric: AttachPort called");

                if state.workers.contains_key(&port_name) {
                    info!(port = %port_name, "ModbusFabric: worker already exists, skipping");
                } else {
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

                    info!(port = %port_name, "ModbusFabric: worker spawned");
                    state.workers.insert(port_name.clone(), worker_ref);

                    // Add default device entry for this port (you may adjust default slave/address)
                    state.devices.push(ModbusDevice {
                        port: port_name.to_string(),
                        slave: 1,    // default slave
                        addres: 0,   // default address to read from
                        value: 0,
                    });
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
                let _ = reply_to.send_message(IpcHandlerMsg::DevicesList(devices_clone));
            }

            ModbusFabricMsg::WriteDevice { device_idx, value } => {
                if let Some(dev) = state.devices.get_mut(device_idx) {
                    dev.value = value;
                    info!("ModbusFabric: device idx {} updated = {}", device_idx, value);
                } else {
                    warn!("ModbusFabric: WriteDevice: index {} out of range", device_idx);
                }
            }

            ModbusFabricMsg::Tick => {
                // Iterate devices and request read from relevant worker.
                // We will create a small response actor for each request that will
                // forward the result back to this Fabric as ModbusFabricMsg::ReadResult.
                for (idx, dev) in state.devices.iter().enumerate() {
                    let port = SmolStr::from(dev.port.clone());
                    if let Some(worker) = state.workers.get(&port) {
                        // Build Modbus read frame for function 0x04 (slave, func, addr hi, addr lo, cnt hi, cnt lo, CRC..)
                        // TODO: build proper RTU frame with CRC. Here we assume worker expects complete frame bytes.
                        // Example (without CRC): [slave, func, addr_hi, addr_lo, cnt_hi, cnt_lo]
                        let addr = dev.addres;
                        let count: u16 = 1; // reading 1 register by default
                        let mut frame = vec![
                            dev.slave,
                            0x04,
                            ((addr >> 8) & 0xFF) as u8,
                            (addr & 0xFF) as u8,
                            ((count >> 8) & 0xFF) as u8,
                            (count & 0xFF) as u8,
                        ];
                        // NOTE: worker needs CRC handling (either here or inside worker/job)
                        // If worker expects full RTU frame including CRC, compute and push CRC here.
                        // For now, we assume worker.send_and_read knows how to handle APDU -> RTU.

                        // Create small response handler actor to receive reply from worker
                        // and forward to Fabric as ModbusFabricMsg::ReadResult
                        let fabric_ref = myself.clone();
                        let idx_copy = idx;
                        // spawn a small one-off actor that expects ModbusReplyBatchMsg
                        let (resp_actor, _jh) = ractor::Actor::spawn(
                            None,
                            ResponseActor::new(),
                            (fabric_ref.clone(), idx_copy),
                        )
                        .await
                        .map_err(|e| ActorProcessingErr::from(e.to_string()))?;

                        // send ScanBatch to worker; worker will reply to resp_actor (ReplyBatch)
                        let _ = worker.cast(ModbusWorkerMsg::ScanBatch {
                            cmds: vec![frame],
                            reply_to: resp_actor,
                        });
                    }
                }

                // re-schedule tick
                let _ = myself.send_after(std::time::Duration::from_secs(2), || ModbusFabricMsg::Tick);
            }

            ModbusFabricMsg::ReadResult { device_idx, value } => {
                if let Some(dev) = state.devices.get_mut(device_idx) {
                    dev.value = value;
                    info!(idx = device_idx, value = value, "ModbusFabric: read result applied");
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

pub struct ResponseActor {
    // state: (fabric_ref, device_idx)
}

impl ResponseActor {
    pub fn new() -> Self {
        Self {}
    }
}

#[ractor::async_trait]
impl Actor for ResponseActor {
    type Msg = ModbusReplyBatchMsg;
    type State = (ActorRef<ModbusFabricMsg>, usize);
    type Arguments = (ActorRef<ModbusFabricMsg>, usize);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(args)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusReplyBatchMsg::ReplyBatch(mut items) => {
                // items: Vec<(cmd, data)>
                // Parse first response into u16, if possible
                if let Some((_cmd, data)) = items.pop() {
                    // parse according to Modbus function 0x04 response format:
                    // Byte 0: byte count (N), followed by N data bytes (registers hi-lo)
                    // But actual format depends on device. Here we try to parse first two bytes as u16.
                    if data.len() >= 2 {
                        let val = ((data[0] as u16) << 8) | (data[1] as u16);
                        let _ = state.0.send_message(ModbusFabricMsg::ReadResult {
                            device_idx: state.1,
                            value: val,
                        });
                    } else {
                        // can't parse -> ignore or send zero
                        let _ = state.0.send_message(ModbusFabricMsg::ReadResult {
                            device_idx: state.1,
                            value: 0,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

// placeholder to satisfy borrow checker in pre_start (no-op)
fn _myself_placeholder() {}
