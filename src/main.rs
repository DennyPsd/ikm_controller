mod actors;

use actors::modbus_fabric_actor::{ModbusDevice, ModbusFabricActor, ModbusFabricMsg};
use ractor::Actor;
use serde::Deserialize;
use std::{collections::HashMap, fs};

#[derive(Debug, Deserialize)]
pub struct Settings {
    devices: Vec<ModbusDevice>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    
    // Читаем файл settings.yaml
    let content = fs::read_to_string("settings.yaml")?;
    let settings: Settings = serde_yaml::from_str(&content)?;
    println!("Loaded devices from settings.yaml: {:?}", settings.devices);

    let mut devices_map = HashMap::new();
    for dev in settings.devices {
        devices_map.insert(dev.id, dev);
    }

    // Создаем актор ModbusFabricActor
    let (modbus_fabric, _handle) =
        Actor::spawn(Some("ModbusFabric".into()), ModbusFabricActor::new(HashMap::new()), devices_map).await?;

    // Отправляем сообщение для вывода списка устройств
     modbus_fabric.send_message(ModbusFabricMsg::PrintDevices)?;

    // Записываем значение в устройство с id=1
    modbus_fabric.send_message(ModbusFabricMsg::WriteDevice {
        device_id: 1,
        value: 42,
    }).unwrap();

    // Снова выводим список устройств
    modbus_fabric.send_message(ModbusFabricMsg::PrintDevices).unwrap();
    //modbus_fabric.cast(ModbusFabricMsg::PrintDevices)?;

    Ok(())
}
