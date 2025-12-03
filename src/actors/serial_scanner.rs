//Отслеживает USB-Serial порты, подключает/отключает их, создавая/разрушая ModbusWorker
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serialport::{SerialPortInfo, SerialPortType};
use smol_str::SmolStr;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio_serial::SerialPortBuilderExt;
use tracing::{error, info};

use crate::actors::modbus_fabric::ModbusFabricMsg;
use crate::actors::modbus_types::ModbusSettings;

pub struct SerialScannerState {
    pub fabric: ActorRef<ModbusFabricMsg>,
    pub known: HashMap<SmolStr, SerialPortInfo>,
    pub settings: ModbusSettings,
}

pub struct SerialScannerActor;
impl SerialScannerActor {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub enum SerialScannerMsg {
    Tick,
}

#[ractor::async_trait]
impl Actor for SerialScannerActor {
    type Msg = SerialScannerMsg;
    type State = SerialScannerState;
    type Arguments = (ActorRef<ModbusFabricMsg>, ModbusSettings);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        (fabric, settings): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        // запускаем первый тик сканирования USB-портов
        let _ = myself.cast(SerialScannerMsg::Tick);
        Ok(SerialScannerState {
            fabric,
            known: HashMap::new(),
            settings,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut SerialScannerState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SerialScannerMsg::Tick => {
                let ports = match serialport::available_ports() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("SerialScanner: ошибка available_ports(): {e}");
                        // перезапускаем тик в случае ошибки
                        let _ =
                            myself.send_after(Duration::from_secs(1), || SerialScannerMsg::Tick);
                        return Ok(());
                    }
                };

                let mut seen: HashSet<SmolStr> = HashSet::new();

                for p in ports {
                    let full_path = &p.port_name;

                    // пропускаем /dev/cu.* на macos
                    if cfg!(target_os = "macos") && full_path.starts_with("/dev/cu.") {
                        continue;
                    }

                    // интересуют USB-порты (или можно убрать фильтр и принимать все)
                    let SerialPortType::UsbPort(_usb) = &p.port_type else {
                        continue;
                    };

                    // Создаем ключ USB-порта для сверки запоминания
                    let key = SmolStr::from(full_path.clone());
                    seen.insert(key.clone());

                    if !state.known.contains_key(&key) {
                        info!(%key, "SerialScanner: найден новый USB - пробуем подключиться");

                        // Ищем группу для этого порта
                        let group = state.settings.groups.values().find(|g| g.com_port == key);

                        // Пробуем открыть поток с настройками из YAML (если порт найден в группах, иначе с дефолтными)
                        let bitrate = group.map(|g| g.bitrate).unwrap_or(9600);
                        let mut builder = tokio_serial::new(full_path, bitrate)
                            .timeout(std::time::Duration::from_millis(1500));

                        // Устанавливаем parity
                        let parity_str = group.map(|g| g.parity.as_str()).unwrap_or("none");
                        match parity_str {
                            "none" => {
                                builder = builder.parity(tokio_serial::Parity::None);
                            }
                            "even" => {
                                builder = builder.parity(tokio_serial::Parity::Even);
                            }
                            "odd" => {
                                builder = builder.parity(tokio_serial::Parity::Odd);
                            }
                            _ => {
                                builder = builder.parity(tokio_serial::Parity::None);
                            }
                        }

                        // Устанавливаем data bits
                        let data_bits = group.map(|g| g.data_bits).unwrap_or(8);
                        match data_bits {
                            5 => {
                                builder = builder.data_bits(tokio_serial::DataBits::Five);
                            }
                            6 => {
                                builder = builder.data_bits(tokio_serial::DataBits::Six);
                            }
                            7 => {
                                builder = builder.data_bits(tokio_serial::DataBits::Seven);
                            }
                            8 => {
                                builder = builder.data_bits(tokio_serial::DataBits::Eight);
                            }
                            _ => {
                                builder = builder.data_bits(tokio_serial::DataBits::Eight);
                            }
                        }

                        // Устанавливаем stop bits
                        let stop_bits = group.map(|g| g.stop_bits).unwrap_or(1);
                        match stop_bits {
                            1 => {
                                builder = builder.stop_bits(tokio_serial::StopBits::One);
                            }
                            2 => {
                                builder = builder.stop_bits(tokio_serial::StopBits::Two);
                            }
                            _ => {
                                builder = builder.stop_bits(tokio_serial::StopBits::One);
                            }
                        }

                        match builder.open_native_async() {
                            Ok(stream) => {
                                // сообщаем Fabric, что порт подключился
                                info!(%key, "SerialScanner: отправил сканирование в ModbusFabric");
                                let _res = state.fabric.send_message(ModbusFabricMsg::AttachPort {
                                    port_name: key.clone(),
                                    stream,
                                });
                                //info!(%key, ?res, "SerialScanner: AttachPort send result");
                                state.known.insert(key.clone(), p.clone());
                                //info!(%key, "SerialScanner: attached and informed fabric");
                            }
                            Err(e) => {
                                error!(%key, "SerialScanner: ошибка открытия stream: {e}");
                            }
                        }
                    }
                }

                // отключение и удаление порта
                let existing: Vec<SmolStr> = state.known.keys().cloned().collect();
                for key in existing {
                    if !seen.contains(&key) {
                        let _ = state.fabric.cast(ModbusFabricMsg::DetachPort {
                            port_name: key.clone(),
                        });
                        state.known.remove(&key);
                        info!(%key, "SerialScanner: USB отключен. Передали в ModBusFabric");
                    }
                }

                // перезапустить тик
                let _ = myself.send_after(Duration::from_secs(1), || SerialScannerMsg::Tick);
            }
        }

        Ok(())
    }
}
