// serial_scanner_modbus.rs
use crate::actors::modbus_fabric::ModbusFabricMsg;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serialport::{SerialPortInfo, SerialPortType};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
use tokio_serial::SerialPortBuilderExt;
use tracing::{error, info};

pub struct SerialScannerState {
    pub fabric: ActorRef<ModbusFabricMsg>,
    pub known: HashMap<SmolStr, SerialPortInfo>,
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
    type Arguments = ActorRef<ModbusFabricMsg>;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        fabric: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        // kick off first tick
        let _ = myself.cast(SerialScannerMsg::Tick);
        Ok(SerialScannerState {
            fabric,
            known: HashMap::new(),
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
                        // schedule next tick quickly
                        let _ = myself.send_after(std::time::Duration::from_secs(5), || SerialScannerMsg::Tick);
                        return Ok(());
                    }
                };

                let mut seen: HashSet<SmolStr> = HashSet::new();

                for p in ports {
                    let full_path = &p.port_name;

                    // macOS: skip /dev/cu.* entries if desired
                    if cfg!(target_os = "macos") && full_path.starts_with("/dev/cu.") {
                        continue;
                    }

                    // only USB-type ports
                    let SerialPortType::UsbPort( usb) = &p.port_type else {
                        continue;
                    };

                    let key = SmolStr::from(full_path);
                    seen.insert(key.clone());

                    if !state.known.contains_key(&key) {
                        info!(
                            port = %full_path,
                            vid = format_args!("{:04x}", usb.vid),
                            pid = format_args!("{:04x}", usb.pid),
                            "SerialScanner: USB подключен"
                        );

                        // build serial builder (default 9600; adapt if you need config)
                        let builder = tokio_serial::new(full_path, 9600)
                            .parity(tokio_serial::Parity::None)
                            .stop_bits(tokio_serial::StopBits::One)
                            .data_bits(tokio_serial::DataBits::Eight)
                            .timeout(std::time::Duration::from_millis(100));

                        match builder.open_native_async() {
                            Ok(stream) => {
                                // send AttachPort to ModbusFabricActor
                                let _ = state.fabric.cast(ModbusFabricMsg::AttachPort {
                                    port_name: key.clone(),
                                    stream,
                                });
                                state.known.insert(key.clone(), p.clone());
                            }
                            Err(e) => {
                                error!(port = %full_path, error = %e, "SerialScanner: ошибка открытия потока сообщений");
                            }
                        }
                    }
                }

                // handle detach
                let existing: Vec<SmolStr> = state.known.keys().cloned().collect();
                for key in existing {
                    if !seen.contains(&key) {
                        let _ = state.fabric.cast(ModbusFabricMsg::DetachPort { port_name: key.clone() });
                        state.known.remove(&key);
                        info!(port = %key, "SerialScanner: USB отключен. Дать инфу в fabric");
                    }
                }

                // schedule next tick after 5 seconds
                let _ = myself.send_after(std::time::Duration::from_secs(5), || SerialScannerMsg::Tick);
            }
        }

        Ok(())
    }
}
