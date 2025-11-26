// actors/modbus_worker.rs
use crate::actors::modbus_types::{ModbusReply, ModbusTimings};
use crate::actors::modbus_worker_job::send_and_read;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::time::Duration;
use tokio_serial::SerialStream;
use tracing::{error, info};
use smol_str::SmolStr;
use crate::actors::modbus_fabric::ModbusFabricMsg;

#[derive(Debug)]
pub enum ModbusWorkerMsg {
    Poll,
    Stop,
}

#[derive(Debug)]
pub struct ModbusWorkerState {
    pub stream: Option<SerialStream>,
    pub timings: ModbusTimings,
    pub fabric: ActorRef<ModbusFabricMsg>,
    pub port_name: SmolStr,
    /// default slave if needed
    pub default_slave: u16,
    /// request template (function/reg/quantity) — for now simple: function 4, addr 0, qty 1
    pub reg_addr: u16,
}

pub struct ModbusWorker;

impl ModbusWorker {
    pub fn new() -> Self { Self }
}

#[ractor::async_trait]
impl Actor for ModbusWorker {
    type Msg = ModbusWorkerMsg;
    type State = ModbusWorkerState;
    // Arguments: (timings, stream, fabric_ref, port_name, default_slave)
    type Arguments = (ModbusTimings, SerialStream, ActorRef<ModbusFabricMsg>, SmolStr, u16);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (timings, stream, fabric, port_name, default_slave): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!(port=%port_name, "ModbusWorker: started (stream available)");
        // schedule first poll after 1s
        let _ = myself.send_after(Duration::from_secs(1), || ModbusWorkerMsg::Poll);
        Ok(ModbusWorkerState {
            stream: Some(stream),
            timings,
            fabric,
            port_name,
            default_slave,
            reg_addr: 0,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusWorkerMsg::Poll => {
                // If no stream — skip
                if state.stream.is_none() {
                    let _ = myself.send_after(Duration::from_secs(2), || ModbusWorkerMsg::Poll);
                    return Ok(());
                }

                // take stream to work with send_and_read
                let mut port = state.stream.take().unwrap();

                // Формируем "команду" — у нас заглушка, любая последовательность
                let cmd = vec![0x01, 0x04, (state.reg_addr >> 8) as u8, (state.reg_addr & 0xFF) as u8, 0x00, 0x01];
                let res = send_and_read(&mut port, &cmd, state.timings.first_byte_timeout, state.timings.per_byte_timeout).await;

                match res {
                    Ok(bytes) => {
                        // Преобразуем первые два байта в u16 (Big-endian) — это просто пример
                        let raw = if bytes.len() >= 2 {
                            ((bytes[0] as u16) << 8) | bytes[1] as u16
                        } else {
                            0u16
                        };

                        // Отправляем Fabric отчёт о прочитанном значении
                        let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                            port_name: state.port_name.clone(),
                            slave: state.default_slave,
                            addr: state.reg_addr,
                            raw: Some(raw),
                        });
                    }
                    Err(err) => {
                        error!("ModbusWorker Poll error: {}", err);
                        let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                            port_name: state.port_name.clone(),
                            slave: state.default_slave,
                            addr: state.reg_addr,
                            raw: None,
                        });
                    }
                }

                // return port to state
                state.stream = Some(port);

                // schedule next poll after 2s
                let _ = myself.send_after(Duration::from_secs(2), || ModbusWorkerMsg::Poll);
            }

            ModbusWorkerMsg::Stop => {
                let _ = myself.stop(None);
            }
        }
        Ok(())
    }
}
