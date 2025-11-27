// actors/modbus_worker.rs
use crate::actors::modbus_types::{ModbusTimings};
use crate::actors::modbus_worker_job::{send_and_read, parse_float_swapped};
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
    pub default_slave: u16,
    pub reg_addr: u16, // стартовый адрес для чтения
}

pub struct ModbusWorker;

impl ModbusWorker {
    pub fn new() -> Self { Self }
}

#[ractor::async_trait]
impl Actor for ModbusWorker {
    type Msg = ModbusWorkerMsg;
    type State = ModbusWorkerState;
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
            reg_addr: 50, //ТУТ АДРЕС ДАТЧИКА
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
                // Если нет потока — пропускаем
                if state.stream.is_none() {
                    let _ = myself.send_after(Duration::from_secs(2), || ModbusWorkerMsg::Poll);
                    return Ok(());
                }

                // Берём поток для работы
                let mut port = state.stream.take().unwrap();

                // Формируем Modbus RTU команду для чтения float (4 байта)
                // Пример: slave=1, func=4, addr=0x32, count=2 (чтение float)
                let cmd = vec![state.default_slave as u8, 0x04, (state.reg_addr >> 8) as u8, (state.reg_addr & 0xFF) as u8, 0x00, 0x02];

                let res = send_and_read(
                    &mut port,
                    &cmd,
                    state.timings.first_byte_timeout,
                    state.timings.per_byte_timeout,
                ).await;

                match res {
                    Ok(bytes) => {
                        if bytes.len() >= 7 {
                            let float_bytes = &bytes[3..7]; // 4 байта данных
                            let value = parse_float_swapped(float_bytes);

                            info!(port=%state.port_name, "Read value: {:.2}", value);

                            // Отправляем Fabric отчёт о прочитанном значении
                            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                                port_name: state.port_name.clone(),
                                slave: state.default_slave,
                                addr: state.reg_addr,
                                raw: Some(value), // если нужно хранить как f32
                            });
                        } else {
                            error!(port=%state.port_name, "Received frame too short: {:02X?}", bytes);
                            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                                port_name: state.port_name.clone(),
                                slave: state.default_slave,
                                addr: state.reg_addr,
                                raw: None,
                            });
                        }
                    }
                    Err(err) => {
                        error!(port=%state.port_name, "ModbusWorker Poll error: {}", err);
                        let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                            port_name: state.port_name.clone(),
                            slave: state.default_slave,
                            addr: state.reg_addr,
                            raw: None,
                        });
                    }
                }

                // Возвращаем поток в состояние
                state.stream = Some(port);

                // Планируем следующий опрос через 2 секунды
                let _ = myself.send_after(Duration::from_secs(2), || ModbusWorkerMsg::Poll);
            }

            ModbusWorkerMsg::Stop => {
                println!("Команда на остановку modbus worker");
                let _ = myself.stop(None);
            }
        }

        Ok(())
    }
}
