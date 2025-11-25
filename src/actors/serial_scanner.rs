use crate::actors::modbus_fabric_actor::ModbusFabricMsg;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serialport::{SerialPortInfo, SerialPortType};
use smol_str::SmolStr;
use std::{collections::{HashMap, HashSet}, time::Duration};
use tokio_serial::SerialPortBuilderExt;
use tracing::{error, info};

pub struct SerialScannerState {
    pub fabric: ActorRef<ModbusFabricMsg>,
    pub known: HashMap<SmolStr, SerialPortInfo>,
}

pub struct SerialScannerActor;
impl SerialScannerActor {
    pub fn new() -> Self { Self }
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
        _myself: ActorRef<Self::Msg>,
        fabric: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(SerialScannerState {
            fabric,
            known: HashMap::new(),
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SerialScannerMsg::Tick => {
                let ports = match serialport::available_ports() {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!("SerialScanner: available_ports() failed: {e}");
                        let _ = myself.send_after(Duration::from_secs(5), || SerialScannerMsg::Tick);
                        return Ok(());
                    }
                };

                let mut seen: std::collections::HashSet<SmolStr> = std::collections::HashSet::new();

                for p in ports {
                    let key = SmolStr::from(&p.port_name);
                    seen.insert(key.clone());

                    if !state.known.contains_key(&key) {
                        // Отправка AttachPort в ModbusFabricActor
                        if let Ok(stream) = tokio_serial::new(&p.port_name, 9600).open_native_async() {
                            let _ = state.fabric.cast(ModbusFabricMsg::AttachPort {
                                port_name: key.clone(),
                                stream,
                            });
                            state.known.insert(key.clone(), p.clone());
                        }
                    }
                }

                // Проверка отключений
                let existing: Vec<SmolStr> = state.known.keys().cloned().collect();
                for key in existing {
                    if !seen.contains(&key) {
                        let _ = state.fabric.cast(ModbusFabricMsg::DetachPort { port_name: key.clone() });
                        state.known.remove(&key);
                    }
                }

                // Планируем следующий тик через 5 секунд
                let _ = myself.send_after(Duration::from_secs(5), || SerialScannerMsg::Tick);
            }
        }

        Ok(())
    }
}