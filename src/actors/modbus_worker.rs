// Формирует команду Мультиплексору и получает ответ
//TODO: решить вопрос с send_after (слишком часто)
use crate::actors::modbus_fabric::ModbusFabricMsg;
use crate::actors::modbus_types::{ModbusTimings, SensorConfig};
use crate::actors::modbus_worker_job::{parse_float_swapped, send_and_read};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use smol_str::SmolStr;
use std::time::Duration;
use tokio_serial::SerialStream;
use tracing::{error, info};

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
    pub sensors: Vec<SensorConfig>,
    pub current_sensor_index: usize,
    pub polling_ms: u64,
}

pub struct ModbusWorker;

impl ModbusWorker {
    pub fn new() -> Self {
        Self
    }
}

#[ractor::async_trait]
impl Actor for ModbusWorker {
    type Msg = ModbusWorkerMsg;
    type State = ModbusWorkerState;
    type Arguments = (
        ModbusTimings,
        SerialStream,
        ActorRef<ModbusFabricMsg>,
        SmolStr,
        Vec<SensorConfig>,
        u64,
    );

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (timings, stream, fabric, port_name, sensors, polling_ms): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!(port=%port_name, "ModbusWorker: запущен (stream активно)");
        // Запускаем считываение чз 1с после старта воркера
        let _ = myself.send_after(Duration::from_secs(1), || ModbusWorkerMsg::Poll);

        Ok(ModbusWorkerState {
            stream: Some(stream),
            timings,
            fabric,
            port_name,
            sensors,
            current_sensor_index: 0,
            polling_ms,
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

                let sensor = &state.sensors[state.current_sensor_index];

                // Формируем Modbus RTU команду для чтения float (4 байта)
                let cmd = vec![
                    sensor.slave,
                    sensor.reg_type,
                    (sensor.start_reg >> 8) as u8,
                    (sensor.start_reg & 0xFF) as u8,
                    0x00,
                    0x02,
                ];

                let res = send_and_read(
                    &mut port,
                    &cmd,
                    state.timings.first_byte_timeout,
                    state.timings.per_byte_timeout,
                )
                .await;

                match res {
                    Ok(bytes) => {
                        if bytes.len() >= 7 {
                            let float_bytes = &bytes[3..7]; // 4 байта данных
                            let value = parse_float_swapped(float_bytes);

                            info!(port=%state.port_name, "Прочитанное значение датчика: {:.2}", value);

                            // Отправляем Fabric отчёт о прочитанном значении
                            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                                port_name: state.port_name.clone(),
                                slave: sensor.slave as u16,
                                addr: sensor.start_reg,
                                raw: Some(value), // если нужно хранить как f32
                            });
                        } else {
                            error!(port=%state.port_name, "пришло слишком короткое сообщение: {:02X?}", bytes);
                            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                                port_name: state.port_name.clone(),
                                slave: sensor.slave as u16,
                                addr: sensor.start_reg,
                                raw: None,
                            });
                        }
                    }
                    Err(err) => {
                        error!(port=%state.port_name, "ModbusWorker Poll error: {}", err);
                        let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                            port_name: state.port_name.clone(),
                            slave: sensor.slave as u16,
                            addr: sensor.start_reg,
                            raw: None,
                        });
                    }
                }

                // Возвращаем поток в состояние
                state.stream = Some(port);

                // Переходим к следующему сенсору
                state.current_sensor_index = (state.current_sensor_index + 1) % state.sensors.len();

                // Планируем следующий опрос через polling_ms миллисекунд
                let _ = myself.send_after(Duration::from_millis(state.polling_ms), || {
                    ModbusWorkerMsg::Poll
                });
            }

            ModbusWorkerMsg::Stop => {
                println!("Команда на остановку modbus worker");
                let _ = myself.stop(None);
            }
        }

        Ok(())
    }
}
