use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap};

// Устройство ModBus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusDevice {
pub id: u32,
pub value: u16,
}

// Сообщения актору
#[derive(Debug)]
pub enum ModbusFabricMsg {
PrintDevices,
WriteDevice { device_id: u32, value: u16 },
}

pub struct ModbusFabricActor {
devices: HashMap<u32, ModbusDevice>,
}

impl ModbusFabricActor {
pub fn new(devices: HashMap<u32, ModbusDevice>) -> Self {
        Self { devices }
    }
}

#[ractor::async_trait]
impl Actor for ModbusFabricActor {
type Msg = ModbusFabricMsg;
type State = Self;
type Arguments = HashMap<u32,ModbusDevice>;

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
            for device in state.devices.values() {
                println!("Device {} => value={}", device.id, device.value);
            }
        }
        ModbusFabricMsg::WriteDevice { device_id, value } => {
            if let Some(dev) = state.devices.get_mut(&device_id) {
                dev.value = value;
                println!("Device {} updated to {}", device_id, value);
            } else {
                println!("Device {} not found!", device_id);
            }
        }
    }
    Ok(())
}

}
