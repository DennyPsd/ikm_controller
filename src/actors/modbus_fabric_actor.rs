// Работа со списком устройств ModBus (вывод, изменение)
// TODO: Сделать запрос в calc-модуль. Только хз какой calc будет
use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::{collections::HashMap};

use crate::actors::ipc_handler::IpcHandlerMsg;

// Устройство ModBus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusDevice {
    pub port: String,
    pub slave: u8,
    pub addres: u16,
    pub value: u16,
}

// Сообщения актору
#[derive(Debug)]
pub enum ModbusFabricMsg {
PrintDevices,
WriteDevice { port: String, slave: u8, addres: u16, value: u16 },
GetDevices(ActorRef<IpcHandlerMsg>),
AttachPort {port_name:SmolStr, stream: tokio_serial::SerialStream,},
DetachPort {port_name:SmolStr},
}

pub struct ModbusFabricActor {
    pub devices: HashMap<(String, u8, u16), ModbusDevice>,
}

impl ModbusFabricActor {
    pub fn new(devices: Vec<ModbusDevice>) -> Self {
        let mut map = HashMap::new();
        for dev in devices {
            map.insert((dev.port.clone(), dev.slave, dev.addres), dev);
        }
        Self { devices: map }
    }
}

#[ractor::async_trait]
impl Actor for ModbusFabricActor {
type Msg = ModbusFabricMsg;
type State = Self;
type Arguments = Vec<ModbusDevice>;

async fn pre_start(
    &self,
    _myself: ActorRef<Self::Msg>,
    args: Self::Arguments,
) -> Result<Self::State, ActorProcessingErr> {
    Ok(ModbusFabricActor::new(args))
}

async fn handle(
    &self,
    _myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut Self::State,
) -> Result<(), ActorProcessingErr> {
    match msg {
        ModbusFabricMsg::PrintDevices => {
            println!("--- Devices list ---");
            for dev in state.devices.values() {
                println!("Device {:?} => value={}", dev, dev.value);
            }
        }
        ModbusFabricMsg::WriteDevice { port, slave, addres, value } => {
                if let Some(dev) = state.devices.get_mut(&(port.clone(), slave, addres)) {
                    dev.value = value;
                    println!("Device {:?} updated to {}", dev, value);
                }
            }
        ModbusFabricMsg::GetDevices(sender) => {
        let list = state.devices.values().cloned().collect();
        let _ = sender.send_message(IpcHandlerMsg::DevicesList(list));
        }
        ModbusFabricMsg::AttachPort { port_name, stream } => {
            print!("{}", port_name);
        }
        ModbusFabricMsg::DetachPort { port_name } => {}

    }
    Ok(())
}

}
